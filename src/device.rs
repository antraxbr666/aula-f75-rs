//! Device discovery and block read/write over the vendor HID channel.

use std::thread;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use hidapi::{HidApi, HidDevice};
use std::ffi::CString;

use crate::protocol::*;

/// Supported transports (wired 258A:010C is a different, undocumented transport).
pub const SUPPORTED: [(u16, u16, &str); 2] = [
    (0x3554, 0xFA09, "AULA F75 (2.4G dongle)"),
    (0x3554, 0xFA07, "AULA F75 (Bluetooth)"),
];

const USAGE_PAGE_VENDOR: u16 = 0xFF02;
const WRITE_GAP: Duration = Duration::from_millis(15);
const READ_TIMEOUT_MS: i32 = 500;

pub struct Keyboard {
    dev: HidDevice,
    pub name: &'static str,
}

/// Print every supported device currently connected.
pub fn list(api: &HidApi) {
    let mut found = false;
    for (vid, pid, name) in SUPPORTED {
        for info in api.device_list() {
            if info.vendor_id() == vid && info.product_id() == pid {
                println!(
                    "{name}\n  vid:pid = {vid:04X}:{pid:04X}  usage_page = 0x{:04X}  path = {}",
                    info.usage_page(),
                    info.path().to_string_lossy()
                );
                found = true;
            }
        }
    }
    if !found {
        println!("No AULA F75 found (supported: 2.4G dongle or Bluetooth).");
    }
}

impl Keyboard {
    /// Open the vendor configuration collection of the first supported device.
    pub fn open(api: &HidApi) -> Result<Keyboard> {
        for (vid, pid, name) in SUPPORTED {
            // Prefer the vendor usage-page collection; fall back to any collection.
            let mut fallback: Option<CString> = None;
            for info in api.device_list() {
                if info.vendor_id() == vid && info.product_id() == pid {
                    if info.usage_page() == USAGE_PAGE_VENDOR {
                        let dev = api
                            .open_path(info.path())
                            .context("failed to open device")?;
                        return Ok(Keyboard { dev, name });
                    }
                    fallback.get_or_insert(info.path().to_owned());
                }
            }
            if let Some(path) = fallback {
                let dev = api.open_path(&path).context("failed to open device")?;
                return Ok(Keyboard { dev, name });
            }
        }
        bail!("no AULA F75 found via 2.4G dongle or Bluetooth (wired mode is not supported yet)")
    }

    fn write_packet(&self, pkt: &[u8]) -> Result<()> {
        let n = self
            .dev
            .write(pkt)
            .with_context(|| format!("write failed for packet idx {}", pkt[3]))?;
        if n != pkt.len() {
            bail!("short write: {n} of {}", pkt.len());
        }
        Ok(())
    }

    /// Read one packet (20 bytes) from the device.
    fn read_packet(&self) -> Result<Option<Vec<u8>>> {
        let mut buf = [0u8; PACKET_LEN + 1]; // +1: report id on Windows
        let n = self.dev.read_timeout(&mut buf, READ_TIMEOUT_MS)?;
        if n == 0 {
            return Ok(None);
        }
        Ok(Some(buf[..n].to_vec()))
    }

    /// Write a whole block, split into 14-byte chunks.
    /// The keyboard acknowledges every packet by echoing it back.
    pub fn write_block(&self, cmd: u8, sub: u8, data: &[u8]) -> Result<()> {
        let chunks: Vec<&[u8]> = data.chunks(PAYLOAD_MAX).collect();
        let count = chunks.len() as u8;
        for (idx, chunk) in chunks.iter().enumerate() {
            let pkt = make_packet(cmd, count, idx as u8, sub, chunk);
            self.write_packet(&pkt)?;
            // Wait for the echo acknowledgement.
            match self.read_packet()? {
                Some(echo) if echo.get(2) == Some(&count) && echo.get(3) == Some(&(idx as u8)) => {
                    if echo[2] & 0x80 != 0 {
                        bail!("keyboard reported error (packet count has bit 7 set) for packet {idx}: {echo:02X?}");
                    }
                }
                Some(other) => bail!("unexpected acknowledgement for packet {idx}: {other:02X?}"),
                None => bail!("no acknowledgement for packet {idx} (keyboard asleep? press a key and retry)"),
            }
            thread::sleep(WRITE_GAP);
        }
        Ok(())
    }

    /// Read a whole block of `expected_len` bytes, reassembling reply
    /// packets by index. Retries missing chunks.
    pub fn read_block(&self, write_cmd: u8, sub: u8, expected_len: usize) -> Result<Vec<u8>> {
        let total_packets = expected_len.div_ceil(PAYLOAD_MAX) as u8;
        let mut out = vec![0u8; expected_len];
        let mut have = vec![false; total_packets as usize];
        let request = make_packet(cmd::read(write_cmd), 1, 0, sub, &[]);

        for _attempt in 0..4 {
            self.write_packet(&request)?;
            // Collect every packet already queued.
            while let Some(pkt) = self.read_packet()? {
                let Some(&count) = pkt.get(2) else { continue };
                if count & 0x80 != 0 {
                    bail!("keyboard reported error for read 0x{:02X}", cmd::read(write_cmd));
                }
                let Some(&idx) = pkt.get(3) else { continue };
                let Some(&len) = pkt.get(4) else { continue };
                let len = (len & 0x0F) as usize;
                if idx as usize >= have.len() {
                    continue;
                }
                let start = idx as usize * PAYLOAD_MAX;
                let end = (start + len).min(expected_len);
                out[start..end].copy_from_slice(&pkt[5..5 + (end - start)]);
                have[idx as usize] = true;
                if have.iter().all(|h| *h) {
                    return Ok(out);
                }
            }
            // Some chunks missing: the keyboard always resends the whole
            // reply on a repeated request, so just ask again.
        }
        let missing: Vec<usize> = have
            .iter()
            .enumerate()
            .filter(|(_, h)| !**h)
            .map(|(i, _)| i)
            .collect();
        bail!("read 0x{:02X} incomplete, missing packets {missing:?}", cmd::read(write_cmd))
    }

    /// Read the 128-byte LED config block.
    pub fn read_led(&self) -> Result<Vec<u8>> {
        let block = self.read_block(cmd::LED, 0, BLOCK_LEN_LED)?;
        if block[126..128] != led::SIGNATURE {
            bail!(
                "LED block signature mismatch: {:02X?} (expected 5A A5) — wrong transport?",
                &block[126..128]
            );
        }
        Ok(block)
    }

    /// Read-modify-write the LED config block.
    pub fn write_led(&self, block: &[u8]) -> Result<()> {
        self.write_block(cmd::LED, 0, block)
    }

    /// Battery percentage (read from the profile block, 0x4A — first byte,
    /// unconfirmed at low charge).
    pub fn battery(&self) -> Result<Option<u8>> {
        let block = self.read_block(cmd::PROFILE, 0, BLOCK_LEN_PROFILE)?;
        Ok(block.first().copied())
    }
}
