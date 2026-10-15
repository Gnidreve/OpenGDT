//! Minimaler PNG-Encoder (8 Bit, Graustufen/RGB) mit „stored“-Deflate-Blöcken.
//! Keine Kompression – dafür trivial korrekt und ohne Abhängigkeiten.

#[derive(Clone, Copy)]
pub enum Color {
    Gray,
    Rgb,
}

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (n, slot) in table.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *slot = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, ty: &[u8; 4], body: &[u8]) {
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    let mut crc_in = ty.to_vec();
    crc_in.extend_from_slice(body);
    out.extend_from_slice(&crc_in);
    out.extend_from_slice(&crc32(&crc_in).to_be_bytes());
}

pub fn encode(width: usize, height: usize, color: Color, pixels: &[u8]) -> Vec<u8> {
    let (ctype, bpp) = match color {
        Color::Gray => (0u8, 1),
        Color::Rgb => (2u8, 3),
    };
    debug_assert_eq!(pixels.len(), width * height * bpp);
    // Scanlines mit Filter-Typ 0
    let mut raw = Vec::with_capacity(height * (width * bpp + 1));
    for row in pixels.chunks(width * bpp) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    // zlib-Container mit stored-Blöcken
    let mut z = vec![0x78, 0x01];
    let mut blocks = raw.chunks(65535).peekable();
    if blocks.peek().is_none() {
        z.extend_from_slice(&[1, 0, 0, 0xFF, 0xFF]);
    }
    while let Some(b) = blocks.next() {
        z.push(blocks.peek().is_none() as u8);
        z.extend_from_slice(&(b.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(b.len() as u16)).to_le_bytes());
        z.extend_from_slice(b);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, ctype, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}
