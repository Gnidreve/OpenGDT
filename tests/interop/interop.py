"""Interop-Test: pynetdicom als unabhängige DICOM-Gegenstelle gegen die laufende Bridge.

Aufruf: interop.py ARBEITSVERZEICHNIS   (siehe run.sh, das alles vorbereitet)
"""
import io, os, sys, time, glob
from pydicom.dataset import Dataset, FileMetaDataset
from pydicom.uid import generate_uid, ExplicitVRLittleEndian, JPEGBaseline8Bit, ImplicitVRLittleEndian
from pydicom.encaps import encapsulate
from pynetdicom import AE, debug_logger
from pynetdicom.sop_class import Verification, ModalityWorklistInformationFind, UltrasoundImageStorage, UltrasoundMultiFrameImageStorage
import av
from PIL import Image
import numpy as np

SP = sys.argv[1]
ae = AE(ae_title="MINDRAY")
ae.add_requested_context(Verification)
ae.add_requested_context(ModalityWorklistInformationFind)
for ts in (JPEGBaseline8Bit, ExplicitVRLittleEndian, ImplicitVRLittleEndian):
    ae.add_requested_context(UltrasoundImageStorage, [ts])
for ts in (JPEGBaseline8Bit, ExplicitVRLittleEndian):
    ae.add_requested_context(UltrasoundMultiFrameImageStorage, [ts])

# Ziel ist normalerweise die Bridge selbst; tests/orthanc/run.sh richtet es per Umgebung auf Orthanc aus.
PORT = int(os.environ.get("OPENGDT_DICOM_PORT", "11112"))
CALLED = os.environ.get("OPENGDT_DICOM_AE", "OPENGDT")
assoc = ae.associate("127.0.0.1", PORT, ae_title=CALLED)
# Worklist kann an einem anderen Server abgefragt werden als der Bildversand (z. B. Worklist → Bridge, Bilder → Orthanc)
WL_PORT = int(os.environ.get("OPENGDT_WORKLIST_PORT", PORT))
WL_CALLED = os.environ.get("OPENGDT_WORKLIST_AE", CALLED)
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
wl = assoc if (WL_PORT, WL_CALLED) == (PORT, CALLED) else ae.associate("127.0.0.1", WL_PORT, ae_title=WL_CALLED)
assert wl.is_established, "Worklist-Association fehlgeschlagen"
for status, ident in wl.send_c_find(q, ModalityWorklistInformationFind):
    print("FIND status", hex(status.Status) if status else None)
    if status.Status in (0xFF00, 0xFF01):
        results.append(ident)
if wl is not assoc:
    wl.release()
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

# --- Cine 1: Multi-Frame, JPEG Baseline (10 Frames, Farbverlauf wandert) ---
def cine_base(nr):
    ds = base_ds(generate_uid(), nr)
    ds.SOPClassUID = UltrasoundMultiFrameImageStorage
    ds.Rows = 48; ds.Columns = 64
    ds.FrameTime = "50"
    ds.FrameIncrementPointer = 0x00181063
    return ds

jpegs = []
for i in range(10):
    arr = np.zeros((48, 64, 3), dtype="uint8")
    arr[..., 0] = i * 25          # Rot steigt mit dem Frame-Index
    arr[..., 1] = 100
    arr[..., 2] = 200 - i * 20
    b = io.BytesIO(); Image.fromarray(arr, "RGB").save(b, "JPEG", quality=95)
    jpegs.append(b.getvalue())
ds3 = cine_base(3)
ds3.NumberOfFrames = len(jpegs)
ds3.PixelData = encapsulate(jpegs)
ds3["PixelData"].is_undefined_length = True
ds3.file_meta.TransferSyntaxUID = JPEGBaseline8Bit
ds3.is_little_endian = True; ds3.is_implicit_VR = False

# --- Cine 2: Multi-Frame, unkomprimiert Graustufen (8 Frames, Pixelwert = Frame*30 + Spalte) ---
ds4 = cine_base(4)
ds4.SamplesPerPixel = 1; ds4.PhotometricInterpretation = "MONOCHROME2"
ds4.FrameTime = "100"
ds4.NumberOfFrames = 8
gpix = np.zeros((8, 48, 64), dtype="uint8")
for f in range(8):
    gpix[f] = (f * 30 + np.arange(64)[None, :]) % 256
ds4.PixelData = gpix.tobytes()
ds4.file_meta.TransferSyntaxUID = ExplicitVRLittleEndian
ds4.is_little_endian = True; ds4.is_implicit_VR = False
if "PlanarConfiguration" in ds4: del ds4.PlanarConfiguration

for ds in (ds1, ds2, ds3, ds4):
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
# --- Cine-Videos: mit ffmpeg (PyAV) dekodieren ---
avis = [e for e in exports if e.endswith(".avi")]
assert len(avis) == 2, avis
mjpg = [a for a in avis if "003_" in a][0]
raw = [a for a in avis if "004_" in a][0]

with av.open(mjpg) as c:
    v = c.streams.video[0]
    print("MJPG:", v.codec_context.name, v.width, v.height, float(v.average_rate), "fps, Frames:", v.frames)
    assert v.codec_context.name == "mjpeg" and (v.width, v.height) == (64, 48)
    assert abs(float(v.average_rate) - 20.0) < 0.01, "FrameTime 50 ms = 20 fps"
    frames = [f.to_ndarray(format="rgb24") for f in c.decode(video=0)]
assert len(frames) == 10, len(frames)
for i, f in enumerate(frames):    # Rot steigt linear → Reihenfolge und Inhalt stimmen
    assert abs(int(f[..., 0].mean()) - i * 25) <= 6, (i, f[..., 0].mean())

with av.open(raw) as c:
    v = c.streams.video[0]
    print("RAW:", v.codec_context.name, v.width, v.height, float(v.average_rate), "fps, Frames:", v.frames)
    assert (v.width, v.height) == (64, 48) and abs(float(v.average_rate) - 10.0) < 0.01
    assert v.codec_context.pix_fmt == "pal8", v.codec_context.pix_fmt
    # pal8: Ebene 0 enthält die Palettenindizes = exakt unsere Rohpixel (die Konvertierung nach Grau
    # durch swscale würde um ±1 abweichen und ist hier nicht Prüfgegenstand)
    gray = []
    for fr in c.decode(video=0):
        pl = fr.planes[0]
        rows = np.frombuffer(pl, dtype="uint8").reshape(-1, abs(pl.line_size))
        if pl.line_size < 0:      # bottom-up-DIB: ffmpeg liefert negativen Zeilenabstand → Zeilen umdrehen
            rows = rows[::-1]
        gray.append(rows[:48, :64])
assert len(gray) == 8, len(gray)
for f in range(8):                # exakte Pixelwerte, Zeilenreihenfolge (bottom-up korrekt gedreht)
    assert (gray[f] == gpix[f]).all(), f
print("INTEROP OK")
