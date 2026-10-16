#!/usr/bin/env python3
"""Tests für check_sources.py (nur Standardbibliothek): lokaler HTTP-Server statt Mindray.

  python3 -I devices/test_check_sources.py
"""
import hashlib
import http.server
import json
import os
import sys
import tempfile
import threading
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import check_sources as cs  # noqa: E402

PDF_V1 = b"%PDF-1.4\nerste Fassung\n%%EOF\n"
PDF_V2 = b"%PDF-1.4\nzweite, laengere Fassung\n%%EOF\n"

PROFILE = """{
  "meta": {
    "schema": "opengdt-device-profile/1",
    "profile_id": "test-geraet",
    "source": {
      "url": "%(url)s",
      "file_sha256": "%(sha)s",
      "file_bytes": %(size)d,
      "pages": 3
    },
    "capture": {
      "extracted_at": "2026-01-01T00:00:00Z",
      "review_status": "reviewed",
      "reviewed_by": "Dr. Test",
      "reviewed_at": "2026-02-02T00:00:00Z",
      "verified_against_device": false
    }
  },
  "rest": {"unveraendert": [1, 2,   3]}
}
"""


class Handler(http.server.BaseHTTPRequestHandler):
    body = PDF_V1
    status = 200

    def do_GET(self):
        self.send_response(Handler.status)
        self.end_headers()
        self.wfile.write(Handler.body)

    def log_message(self, *a):
        pass


class CheckSources(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.srv = http.server.HTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=cls.srv.serve_forever, daemon=True).start()
        cls.url = f"http://127.0.0.1:{cls.srv.server_port}/x.pdf"

    @classmethod
    def tearDownClass(cls):
        cls.srv.shutdown()
        cls.srv.server_close()

    def setUp(self):
        Handler.body, Handler.status = PDF_V1, 200
        self.dir = tempfile.mkdtemp()
        os.makedirs(os.path.join(self.dir, "geraet"))
        self.path = os.path.join(self.dir, "geraet", "dicom-conformance.json")
        self.write_profile(PDF_V1)
        # Seitenzahl: ohne pdfinfo bzw. mit Dummy-PDF unbekannt → Vergleich entfällt; Test soll das nicht verdecken
        self._pages = cs.pdf_pages
        cs.pdf_pages = lambda data: None

    def tearDown(self):
        cs.pdf_pages = self._pages

    def write_profile(self, data):
        open(self.path, "w", encoding="utf-8").write(
            PROFILE % {"url": self.url, "sha": hashlib.sha256(data).hexdigest(), "size": len(data)}
        )

    def text(self):
        return open(self.path, encoding="utf-8").read()

    def test_unchanged_source_is_ok_and_file_untouched(self):
        before = self.text()
        _, status, _ = cs.check_one(self.path, update=True)
        self.assertEqual(status, cs.OK)
        self.assertEqual(self.text(), before)

    def test_changed_source_is_reported_without_update(self):
        Handler.body = PDF_V2
        before = self.text()
        _, status, msg = cs.check_one(self.path, update=False)
        self.assertEqual(status, cs.CHANGED)
        self.assertIn("Quelle geändert", msg)
        self.assertEqual(self.text(), before, "ohne --update darf nichts geändert werden")

    def test_update_changes_only_the_intended_places(self):
        Handler.body = PDF_V2
        before = self.text()
        _, status, _ = cs.check_one(self.path, update=True)
        self.assertEqual(status, cs.CHANGED)
        after = self.text()
        j = json.loads(after)["meta"]
        self.assertEqual(j["source"]["file_sha256"], hashlib.sha256(PDF_V2).hexdigest())
        self.assertEqual(j["source"]["file_bytes"], len(PDF_V2))
        self.assertEqual(j["source"]["pages"], 3, "Seitenzahl unbekannt → unverändert")
        c = j["capture"]
        self.assertEqual(c["review_status"], "unreviewed")
        self.assertIsNone(c["reviewed_by"])
        self.assertIsNone(c["reviewed_at"])
        self.assertRegex(c["source_changed_at"], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")
        self.assertEqual(c["extracted_at"], "2026-01-01T00:00:00Z", "Zeitpunkt der Abschrift bleibt")
        # Format erhalten: das `rest`-Objekt (mit absichtlich ungewöhnlichen Leerzeichen) ist byte-gleich
        self.assertIn('"rest": {"unveraendert": [1, 2,   3]}', after)
        self.assertEqual(len(after.splitlines()), len(before.splitlines()) + 1)
        # zweiter Lauf: jetzt stimmt alles
        _, status, _ = cs.check_one(self.path, update=True)
        self.assertEqual(status, cs.OK)

    def test_second_change_replaces_source_changed_at(self):
        Handler.body = PDF_V2
        cs.check_one(self.path, update=True)
        Handler.body = PDF_V2 + b"x"
        cs.check_one(self.path, update=True)
        self.assertEqual(self.text().count('"source_changed_at"'), 1)

    def test_wrong_bytes_with_same_hash_is_fixed_quietly(self):
        broken = self.text().replace(f'"file_bytes": {len(PDF_V1)}', '"file_bytes": 1')
        open(self.path, "w", encoding="utf-8").write(broken)
        _, status, _ = cs.check_one(self.path, update=True)
        self.assertEqual(status, cs.FIXED_META)
        j = json.loads(self.text())["meta"]
        self.assertEqual(j["source"]["file_bytes"], len(PDF_V1))
        self.assertEqual(j["capture"]["review_status"], "reviewed", "Inhalt ist gleich geblieben")
        self.assertNotIn("source_changed_at", self.text())

    def test_error_page_is_not_a_pdf(self):
        Handler.body = b"<html>403</html>"
        before = self.text()
        _, status, _ = cs.check_one(self.path, update=True)
        self.assertEqual(status, cs.BAD)
        self.assertEqual(self.text(), before)

    def test_unreachable_source(self):
        Handler.status = 500
        _, status, _ = cs.check_one(self.path, update=True)
        self.assertEqual(status, cs.UNREACHABLE)

    def test_cli_exit_codes_and_repository_profiles_are_found(self):
        self.assertEqual(cs.main(["check", "--devices", self.dir]), 0)
        Handler.body = PDF_V2
        self.assertEqual(cs.main(["check", "--devices", self.dir]), 1)
        found = cs.profiles(os.path.dirname(os.path.abspath(cs.__file__)))
        self.assertGreaterEqual(len(found), 2)


if __name__ == "__main__":
    unittest.main()
