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
    vid: u16,
    pid: u16,
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
                        return Ok(Keyboard {
                            dev,
                            name,
                            vid,
                            pid,
                        });
                    }
                    fallback.get_or_insert(info.path().to_owned());
                }
            }
            if let Some(path) = fallback {
                let dev = api.open_path(&path).context("failed to open device")?;
                return Ok(Keyboard {
                    dev,
                    name,
                    vid,
                    pid,
                });
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
            // Wait for the echo acknowledgement, skipping stale packets
            // left in the queue by an earlier multi-packet read (the
            // keyboard sometimes retransmits parts of a reply after we
            // already finished reassembling it).
            let mut echoed = false;
            for _try in 0..16 {
                match self.read_packet()? {
                    Some(echo)
                        if echo.get(1) == Some(&cmd)
                            && echo.get(2) == Some(&count)
                            && echo.get(3) == Some(&(idx as u8)) =>
                    {
                        if echo[2] & 0x80 != 0 {
                            bail!("keyboard reported error (packet count has bit 7 set) for packet {idx}: {echo:02X?}");
                        }
                        echoed = true;
                        break;
                    }
                    Some(_) => continue, // stale reply packet — drain it
                    None => bail!("no acknowledgement for packet {idx} (keyboard asleep? press a key and retry)"),
                }
            }
            if !echoed {
                bail!("no acknowledgement for packet {idx} within retry budget");
            }
            thread::sleep(WRITE_GAP);
        }
        Ok(())
    }

    /// Discard any packets still queued (duplicates of a finished read).
    fn drain(&self) {
        let mut buf = [0u8; PACKET_LEN + 1];
        for _ in 0..64 {
            match self.dev.read_timeout(&mut buf, 0) {
                Ok(0) | Err(_) => return,
                Ok(_) => {}
            }
        }
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
                    bail!(
                        "keyboard reported error for read 0x{:02X}",
                        cmd::read(write_cmd)
                    );
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
                    self.drain(); // drop duplicate tail packets
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
        bail!(
            "read 0x{:02X} incomplete, missing packets {missing:?}",
            cmd::read(write_cmd)
        )
    }

    /// Vendor/product id of the open transport.
    pub fn vid_pid(&self) -> (u16, u16) {
        (self.vid, self.pid)
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

    /// Indexed HID string descriptor (some firmwares store the version here).
    pub fn indexed_string(&self, index: i32) -> Option<String> {
        self.dev.get_indexed_string(index).ok().flatten()
    }

    /// HID string descriptors (manufacturer / product / serial — the
    /// firmware version often lives in one of them).
    pub fn strings(&self) -> (Option<String>, Option<String>, Option<String>) {
        (
            self.dev.get_manufacturer_string().ok().flatten(),
            self.dev.get_product_string().ok().flatten(),
            self.dev.get_serial_number_string().ok().flatten(),
        )
    }

    /// Factory reset (command 0x06 — gets NO acknowledgement).
    pub fn factory_reset(&self, api: &HidApi) -> Result<()> {
        self.wait_keys_idle(api)?;
        let pkt = make_packet(0x06, 1, 0, 0, &[1]);
        self.write_packet(&pkt)
    }

    /// Battery percentage (read from the profile block, 0x4A — first byte,
    /// unconfirmed at low charge).
    pub fn battery(&self) -> Result<Option<u8>> {
        let block = self.read_block(cmd::PROFILE, 0, BLOCK_LEN_PROFILE)?;
        Ok(block.first().copied())
    }

    /// Read a keymap layer (command 0x41, 504 bytes = 126 entries x 4).
    /// `layer`: 0 = base, 1 = Fn.
    pub fn read_keymap(&self, layer: u8) -> Result<Vec<u8>> {
        self.read_block(cmd::KEYMAP, layer, BLOCK_LEN_KEYMAP)
    }

    /// Write a keymap layer (flash write — keys must be idle).
    pub fn write_keymap(&self, api: &HidApi, layer: u8, block: &[u8]) -> Result<()> {
        self.wait_keys_idle(api)?;
        self.write_block(cmd::KEYMAP, layer, block)
    }

    /// Read the macro store (command 0x43, block 0, 512 bytes).
    pub fn read_macros(&self) -> Result<Vec<u8>> {
        self.read_block(cmd::MACRO, 0, BLOCK_LEN_MACRO)
    }

    /// Write the macro store (flash write — keys must be idle).
    pub fn write_macros(&self, api: &HidApi, block: &[u8]) -> Result<()> {
        self.wait_keys_idle(api)?;
        self.write_block(cmd::MACRO, 0, block)
    }

    /// Send one real-time lighting frame (command 0x88). No acknowledgement
    /// comes back (bit 7 commands get no echo), so nothing is verified; a
    /// 5 ms gap between packets keeps frames from being lost.
    pub fn send_realtime(&self, sub: u8, data: &[u8]) -> Result<()> {
        let chunks: Vec<&[u8]> = data.chunks(PAYLOAD_MAX).collect();
        let count = chunks.len() as u8;
        for (idx, chunk) in chunks.iter().enumerate() {
            let pkt = make_packet(cmd::REALTIME, count, idx as u8, sub, chunk);
            self.write_packet(&pkt)?;
            thread::sleep(Duration::from_millis(realtime::PACKET_GAP_MS));
        }
        Ok(())
    }

    /// Wait until no keys are held and the keyboard has been idle for a
    /// while, so a configuration write cannot make the keyboard drop a
    /// key-release report (which would leave a key auto-repeating).
    pub fn wait_keys_idle(&self, api: &HidApi) -> Result<()> {
        // Find the boot-keyboard collection of the same physical device.
        let kbd = api
            .device_list()
            .find(|i| {
                i.vendor_id() == self.vid
                    && i.product_id() == self.pid
                    && i.usage_page() == 0x0001
                    && i.usage() == 0x06
            })
            .map(|i| {
                api.open_path(i.path())
                    .context("failed to open keyboard collection")
            });
        let Some(kbd) = kbd else {
            eprintln!("warning: keyboard collection not found, skipping idle check");
            return Ok(());
        };
        let kbd = kbd?;
        kbd.set_blocking_mode(false)
            .context("cannot set non-blocking mode")?;

        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let quiet_needed = Duration::from_millis(400);
        let mut quiet_start = std::time::Instant::now();
        let mut held = false;
        let mut buf = [0u8; 32];

        loop {
            match kbd.read(&mut buf) {
                Ok(0) => {} // no data (non-blocking)
                Ok(_) => {
                    // Boot report: [report-id?][mods][reserved][k1..k6]
                    let keys = &buf[2..buf.len().min(8)];
                    let now_held = keys.iter().any(|k| *k != 0 && *k != 0x01); // 0x01 quirk = ErrorRollOver
                    if now_held {
                        held = true;
                        quiet_start = std::time::Instant::now();
                    } else if held {
                        // release observed — restart the quiet window
                        held = false;
                        quiet_start = std::time::Instant::now();
                    }
                }
                Err(_) => return Ok(()), // read error: don't block the command
            }
            if !held && quiet_start.elapsed() >= quiet_needed {
                return Ok(());
            }
            if std::time::Instant::now() > deadline {
                eprintln!("warning: keys still held (or reports keep coming), proceeding anyway");
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}
