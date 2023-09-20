use std::fs::File;

use anyhow::{Error, ensure, anyhow};
use byteorder::{ByteOrder, BigEndian};
use pcap_parser::*;
use pcap_parser::traits::PcapReaderIterator;

#[derive(Debug)]
pub enum ReturnMode {
    Single,
    Dual,
}

#[derive(Debug)]
pub struct PcapInfo {
    pub return_mode: ReturnMode,
    pub frequency: f32, // Hz
}

pub fn parse_packet_info(filename: &str) -> Result<PcapInfo, Error> {
    let file = File::open(filename).unwrap();
    let mut reader = LegacyPcapReader::new(65536, file).expect("LegacyPcapReader");

    let mut frequency: Option<f32> = None;
    let mut return_mode: Option<ReturnMode> = None;

    loop {
        match reader.next() {
            Ok((offset, block)) => {
                match block {
                    PcapBlockOwned::Legacy(packet) => {
                        let ether_type = &packet.data[12..14];
                        if ether_type != &[0x08, 0x00] {
                            // not ipv4
                            reader.consume(offset);
                            continue;
                        }
                        
                        let udp_data = &packet.data[42..];
                        if udp_data.is_empty() {
                            reader.consume(offset);
                            continue;
                        }
                        
                        let first_byte = udp_data[0];
                        if (first_byte < 128 || first_byte == 0xFF) && return_mode.is_none() {
                            // data package
                            let factory_return_mode = udp_data[1205];
                            return_mode = match factory_return_mode {
                                0x01 => Some(ReturnMode::Single),
                                0x02 => Some(ReturnMode::Dual),
                                _ => return Err(anyhow!("unknown return mode: {}", factory_return_mode)),
                            };
                        }
                        if first_byte == 0xA5 && frequency.is_none() {
                            // device package
                            let motor_speed_rpm = BigEndian::read_u16(&udp_data[8..10]);
                            frequency = Some(motor_speed_rpm as f32 / 60.0);
                        }
                    }
                    _ => ()
                }
                if frequency.is_some() && return_mode.is_some() {
                    break;
                }
                reader.consume(offset);
            },
            Err(PcapError::Eof) => break,
            Err(PcapError::Incomplete) => {
                reader.refill().unwrap();
            },
            Err(e) => panic!("error while reading: {:?}", e),
        }
    }

    ensure!(frequency.is_some(), "frequency not found");
    ensure!(return_mode.is_some(), "return mode not found");

    Ok(PcapInfo {
        return_mode: return_mode.unwrap(),
        frequency: frequency.unwrap(),
    })
}
