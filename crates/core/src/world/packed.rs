//! Long arrays of small numbers in saves (the tiles of a map, the regions of
//! the overworld, the explored cells): runs of equal values packed into a
//! base64 string instead of a JSON list of millions of numbers. Lists written
//! by older versions are still read.
//!
//! Use as `#[serde(with = "crate::world::packed")]` on a `Vec<u8>` or `Vec<u16>`.

use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serializer};
use std::fmt;
use std::marker::PhantomData;

/// Marks the packed form (and its version).
const PREFIX: &str = "rle1:";

/// A value that can be packed.
pub trait Cell: Copy + Eq + for<'de> Deserialize<'de> {
    fn to_u32(self) -> u32;
    fn from_u32(v: u32) -> Option<Self>;
}

impl Cell for u8 {
    fn to_u32(self) -> u32 {
        self as u32
    }
    fn from_u32(v: u32) -> Option<Self> {
        u8::try_from(v).ok()
    }
}

impl Cell for u16 {
    fn to_u32(self) -> u32 {
        self as u32
    }
    fn from_u32(v: u32) -> Option<Self> {
        u16::try_from(v).ok()
    }
}

pub fn serialize<S: Serializer, T: Cell>(v: &[T], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&pack(v))
}

pub fn deserialize<'de, D: Deserializer<'de>, T: Cell>(d: D) -> Result<Vec<T>, D::Error> {
    d.deserialize_any(Unpack(PhantomData))
}

struct Unpack<T>(PhantomData<T>);

impl<'de, T: Cell> Visitor<'de> for Unpack<T> {
    type Value = Vec<T>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("packed runs or a list of numbers")
    }

    fn visit_str<E: de::Error>(self, s: &str) -> Result<Vec<T>, E> {
        unpack(s).ok_or_else(|| E::custom("broken packed data"))
    }

    // the plain list of older saves
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
        let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(x) = seq.next_element()? {
            out.push(x);
        }
        Ok(out)
    }
}

/// Runs of equal values as (length, value) varints, in base64.
pub fn pack<T: Cell>(v: &[T]) -> String {
    let mut raw = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let run = v[i..].iter().take_while(|&&x| x == v[i]).count();
        put_varint(&mut raw, run as u32);
        put_varint(&mut raw, v[i].to_u32());
        i += run;
    }
    let mut s = String::with_capacity(PREFIX.len() + raw.len().div_ceil(3) * 4);
    s.push_str(PREFIX);
    base64_encode(&raw, &mut s);
    s
}

pub fn unpack<T: Cell>(s: &str) -> Option<Vec<T>> {
    let raw = base64_decode(s.strip_prefix(PREFIX)?)?;
    let mut out = Vec::new();
    let mut at = 0;
    while at < raw.len() {
        let run = get_varint(&raw, &mut at)? as usize;
        let v = T::from_u32(get_varint(&raw, &mut at)?)?;
        out.resize(out.len() + run, v);
    }
    Some(out)
}

fn put_varint(out: &mut Vec<u8>, mut v: u32) {
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn get_varint(b: &[u8], at: &mut usize) -> Option<u32> {
    let mut v = 0u32;
    for shift in (0..35).step_by(7) {
        let byte = *b.get(*at)?;
        *at += 1;
        v |= ((byte & 0x7f) as u32) << shift;
        if byte < 0x80 {
            return Some(v);
        }
    }
    None
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8], out: &mut String) {
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() {
                out.push(B64[(n >> (18 - 6 * k)) as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
    }
}

fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let s = s.trim_end_matches('=').as_bytes();
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for &ch in s {
        let v = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        };
        acc = acc << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Map {
        #[serde(with = "super")]
        tiles: Vec<u8>,
        #[serde(with = "super")]
        regions: Vec<u16>,
    }

    #[test]
    fn roundtrip() {
        let mut r = crate::rng::Rng::new(1, 2);
        for n in [0usize, 1, 2, 3, 4, 5, 7, 100, 4099] {
            let tiles: Vec<u8> = (0..n).map(|i| (i / 7) as u8 ^ (r.int_n(3) as u8)).collect();
            let regions: Vec<u16> = (0..n).map(|i| (i / 300) as u16 * 1000).collect();
            let m = Map { tiles, regions };
            let json = serde_json::to_string(&m).unwrap();
            assert_eq!(serde_json::from_str::<Map>(&json).unwrap(), m, "{n}");
        }
    }

    #[test]
    fn old_lists_load() {
        let m: Map = serde_json::from_str(r#"{"tiles":[1,2,2],"regions":[0,700]}"#).unwrap();
        assert_eq!(m.tiles, [1, 2, 2]);
        assert_eq!(m.regions, [0, 700]);
    }

    #[test]
    fn base64_matches_the_standard() {
        let mut s = String::new();
        base64_encode(b"Ratas!", &mut s);
        assert_eq!(s, "UmF0YXMh");
        s.clear();
        base64_encode(b"ok", &mut s);
        assert_eq!(s, "b2s=");
        assert_eq!(base64_decode("b2s=").unwrap(), b"ok");
        assert!(unpack::<u8>("rle1:!!").is_none());
        assert!(unpack::<u8>("[]").is_none());
    }
}
