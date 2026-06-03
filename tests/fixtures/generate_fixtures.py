#!/usr/bin/env python3
"""Deterministically generate the binary PDF test fixtures.

Pure Python standard library — no reportlab / pypdf dependency — so the
fixtures are reproducible on any machine and in CI. Run from anywhere:

    python3 tests/fixtures/generate_fixtures.py

Outputs (committed alongside this script):
    tests/fixtures/hello.pdf          — one page, extractable Helvetica text
    tests/fixtures/japanese_form.pdf  — AcroForm with two empty text fields
                                        ("full_name", "city") used by the
                                        Japanese/CJK form-fill round-trip test
                                        (issue #611 acceptance artifact)

These are intentionally minimal, hand-assembled PDFs with a correctly
computed classic xref table — small enough to audit by eye.
"""

from pathlib import Path


def build_pdf(objects: list[bytes]) -> bytes:
    """Assemble a PDF body + classic xref table from 1-indexed object bodies.

    `objects[i]` is the raw content of object `i+1` (without the
    "N 0 obj ... endobj" wrapper). Offsets are computed as bytes are appended.
    """
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")  # binary marker comment
    offsets = []
    for i, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += f"{i} 0 obj\n".encode("latin-1")
        out += body
        out += b"\nendobj\n"

    xref_pos = len(out)
    n = len(objects) + 1
    out += f"xref\n0 {n}\n".encode("latin-1")
    out += b"0000000000 65535 f \n"
    for off in offsets:
        out += f"{off:010d} 00000 n \n".encode("latin-1")
    out += b"trailer\n"
    out += f"<< /Size {n} /Root 1 0 R >>\n".encode("latin-1")
    out += b"startxref\n"
    out += f"{xref_pos}\n".encode("latin-1")
    out += b"%%EOF\n"
    return bytes(out)


def hello_pdf() -> bytes:
    content = b"BT /F1 24 Tf 72 700 Td (Hello, world!) Tj ET\n"
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
        b"/Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
        b"<< /Length " + str(len(content)).encode("latin-1") + b" >>\nstream\n"
        + content + b"endstream",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    ]
    return build_pdf(objects)


def japanese_form_pdf() -> bytes:
    """AcroForm with two empty text fields. Values are filled by the test;
    the fixture only needs to declare the fields so the round-trip can write
    UTF-8/CJK and read it back."""
    objects = [
        # 1: Catalog with AcroForm. NeedAppearances so viewers regenerate the
        # field appearance after a fill; DR provides a default resource font.
        b"<< /Type /Catalog /Pages 2 0 R "
        b"/AcroForm << /Fields [5 0 R 6 0 R] /NeedAppearances true "
        b"/DA (/Helv 0 Tf 0 g) /DR << /Font << /Helv 4 0 R >> >> >> >>",
        # 2: Pages
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        # 3: Page referencing both widget annotations
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
        b"/Annots [5 0 R 6 0 R] "
        b"/Resources << /Font << /Helv 4 0 R >> >> >>",
        # 4: Helvetica (base-14, no embedding needed)
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
        # 5: text field "full_name" (widget+field combined)
        b"<< /Type /Annot /Subtype /Widget /FT /Tx /T (full_name) "
        b"/Rect [72 700 400 720] /DA (/Helv 12 Tf 0 g) /V () /Ff 0 /P 3 0 R >>",
        # 6: text field "city"
        b"<< /Type /Annot /Subtype /Widget /FT /Tx /T (city) "
        b"/Rect [72 660 400 680] /DA (/Helv 12 Tf 0 g) /V () /Ff 0 /P 3 0 R >>",
    ]
    return build_pdf(objects)


def main() -> None:
    here = Path(__file__).resolve().parent
    (here / "hello.pdf").write_bytes(hello_pdf())
    (here / "japanese_form.pdf").write_bytes(japanese_form_pdf())
    print(f"wrote {here / 'hello.pdf'}")
    print(f"wrote {here / 'japanese_form.pdf'}")


if __name__ == "__main__":
    main()
