"""Interop-Test: pynetdicom als unabhängige DICOM-Gegenstelle gegen die laufende Bridge.

Aufruf: interop.py ARBEITSVERZEICHNIS   (siehe run.sh, das alles vorbereitet)
"""
import io, sys, time, glob
from pydicom.dataset import Dataset, FileMetaDataset
from pydicom.uid import generate_uid, ExplicitVRLittleEndian, JPEGBaseline8Bit, ImplicitVRLittleEndian
from pydicom.encaps import encapsulate
from pynetdicom import AE, debug_logger
from pynetdicom.sop_class import Verification, ModalityWorklistInformationFind, UltrasoundImageStorage, UltrasoundMultiFrameImageStorage
from PIL import Image
import numpy as np

SP = sys.argv[1]
ae = AE(ae_title="MINDRAY")
ae.add_requested_context(Verification)
ae.add_requested_context(ModalityWorklistInformationFind)
for ts in (JPEGBaseline8Bit, ExplicitVRLittleEndian, ImplicitVRLittleEndian):
    ae.add_requested_context(UltrasoundImageStorage, [ts])

assoc = ae.associate("127.0.0.1", 11112, ae_title="OPENGDT")
assert assoc.is_established, "Association fehlgeschlagen"
st = assoc.send_c_echo()
print("ECHO", st.Status)
assert st.Status == 0

# --- Worklist ---
q = Dataset()
q.PatientName = ""
q.PatientID = ""
q.PatientBirthDate = ""
q.PatientSex = ""
q.AccessionNumber = ""
q.StudyInstanceUID = ""
sps = Dataset()
sps.Modality = "US"
sps.ScheduledStationAETitle = "MINDRAY"
sps.ScheduledProcedureStepStartDate = ""
sps.ScheduledProcedureStepStartTime = ""
q.ScheduledProcedureStepSequence = [sps]
results = []
for status, ident in assoc.send_c_find(q, ModalityWorklistInformationFind):
    print("FIND status", hex(status.Status) if status else None)
    if status.Status in (0xFF00, 0xFF01):
        results.append(ident)
print("FIND matches:", len(results))
assert len(results) == 1, results
w = results[0]
print(w.PatientName, w.PatientID, w.PatientBirthDate, w.PatientSex, w.AccessionNumber)
assert str(w.PatientName) == "Müller^Jürgen", repr(w.PatientName)
assert w.PatientID == "4711"
study_uid = w.StudyInstanceUID

# --- Store: JPEG baseline (echtes JPEG aus Pillow) ---
def base_ds(sop_uid, nr):
    ds = Dataset()
    ds.file_meta = FileMetaDataset()
    ds.SOPClassUID = UltrasoundImageStorage
    ds.SOPInstanceUID = sop_uid
    ds.PatientName = w.PatientName
    ds.PatientID = w.PatientID
    ds.PatientBirthDate = w.PatientBirthDate
    ds.PatientSex = w.PatientSex
    ds.StudyInstanceUID = study_uid
    ds.SeriesInstanceUID = generate_uid()
    ds.AccessionNumber = w.AccessionNumber
    ds.StudyDate = "20261015"
    ds.StudyTime = "103000"
    ds.Modality = "US"
    ds.InstanceNumber = nr
    ds.SamplesPerPixel = 3
    ds.PhotometricInterpretation = "YBR_FULL_422"
    ds.Rows = 64; ds.Columns = 96
    ds.BitsAllocated = 8; ds.BitsStored = 8; ds.HighBit = 7; ds.PixelRepresentation = 0
    ds.PlanarConfiguration = 0
    return ds

img = Image.fromarray((np.random.rand(64, 96, 3) * 255).astype("uint8"), "RGB")
buf = io.BytesIO(); img.save(buf, "JPEG", quality=90)
jpeg_bytes = buf.getvalue()
ds1 = base_ds(generate_uid(), 1)
ds1.PixelData = encapsulate([jpeg_bytes])
ds1["PixelData"].is_undefined_length = True
ds1.file_meta.TransferSyntaxUID = JPEGBaseline8Bit
ds1.is_little_endian = True; ds1.is_implicit_VR = False

# --- Store: unkomprimiert Graustufen, Explicit VR LE, groß genug für mehrere PDUs ---
ds2 = base_ds(generate_uid(), 2)
ds2.SamplesPerPixel = 1; ds2.PhotometricInterpretation = "MONOCHROME2"
ds2.Rows = 600; ds2.Columns = 800
gray = (np.arange(600*800) % 256).astype("uint8")
ds2.PixelData = gray.tobytes()
ds2.file_meta.TransferSyntaxUID = ExplicitVRLittleEndian
ds2.is_little_endian = True; ds2.is_implicit_VR = False
if "PlanarConfiguration" in ds2: del ds2.PlanarConfiguration

for ds in (ds1, ds2):
    s = assoc.send_c_store(ds)
    print("STORE", hex(s.Status))
    assert s.Status == 0
assoc.release()

# --- Ergebnisse abwarten ---
for _ in range(60):
    g = glob.glob(f"{SP}/gdt-out/*.gdt")
    if g: break
    time.sleep(0.5)
assert g, "keine GDT-Antwort"
gdt = open(g[0], "rb").read().decode("latin-1")
print(gdt.replace("\r\n", "\n"))
exports = sorted(glob.glob(f"{SP}/export/**/*.*", recursive=True))
print(exports)
jpg = [e for e in exports if e.endswith(".jpg")][0]
png = [e for e in exports if e.endswith(".png")][0]
assert open(jpg, "rb").read() == jpeg_bytes
im = Image.open(jpg); im.load(); print("JPG", im.size)
im2 = Image.open(png); im2.load(); print("PNG", im2.size, im2.mode)
assert im2.size == (800, 600)
assert list(im2.crop((0, 0, 5, 1)).tobytes()) == [0, 1, 2, 3, 4]
print("INTEROP OK")
