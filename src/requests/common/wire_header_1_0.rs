// Copyright (c) 2019, Arm Limited, All Rights Reserved
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may
// not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//          http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
// WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! This module defines and implements the raw wire protocol header frame for
//! version 1.0 of the protocol.
use crate::requests::common::MAGIC_NUMBER;
use crate::requests::{ResponseStatus, Result};
#[cfg(feature = "fuzz")]
use arbitrary::Arbitrary;
use log::error;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::io::{Read, Write};

const WIRE_PROTOCOL_VERSION_MAJ: u8 = 1;
const WIRE_PROTOCOL_VERSION_MIN: u8 = 0;

const REQUEST_HDR_SIZE: u16 = 24;

/// Raw representation of a common request/response header, as defined for the wire format.
///
/// Serialisation and deserialisation are handled by `serde`, also in tune with the
/// wire format (i.e. little-endian, native encoding).
#[cfg_attr(feature = "fuzz", derive(Arbitrary))]
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct WireHeader {
    /// Provider ID value
    pub provider: u8,
    /// Session handle
    pub session: u64,
    /// Content type: defines how the request body should be processed.
    pub content_type: u8,
    /// Accept type: defines how the service should provide its response.
    pub accept_type: u8,
    /// Authentication type.
    pub auth_type: u8,
    /// Number of bytes of content.
    pub body_len: u32,
    /// Number of bytes of authentication.
    pub auth_len: u16,
    /// Opcode of the operation to perform.
    pub opcode: u16,
    /// Response status of the request.
    pub status: u16,
}

impl WireHeader {
    /// Create a new raw wire header.
    ///
    /// For use in testing only.
    #[cfg(feature = "testing")]
    #[allow(clippy::new_without_default)]
    pub fn new() -> WireHeader {
        WireHeader {
            provider: 0,
            session: 0,
            content_type: 0,
            accept_type: 0,
            auth_type: 0,
            body_len: 0,
            auth_len: 0,
            opcode: 0,
            status: 0,
        }
    }

    /// Serialise the request header and write the corresponding bytes to the given
    /// stream.
    ///
    /// # Errors
    /// - if marshalling the header fails, `ResponseStatus::InvalidEncoding` is returned.
    /// - if writing the header bytes fails, `ResponseStatus::ConnectionError` is returned.
    pub fn write_to_stream<W: Write>(&self, stream: &mut W) -> Result<()> {
        stream.write_all(&bincode::serialize(&MAGIC_NUMBER)?)?;

        stream.write_all(&bincode::serialize(&REQUEST_HDR_SIZE)?)?;

        stream.write_all(&bincode::serialize(&WIRE_PROTOCOL_VERSION_MAJ)?)?;
        stream.write_all(&bincode::serialize(&WIRE_PROTOCOL_VERSION_MIN)?)?;

        stream.write_all(&bincode::serialize(&self)?)?;

        Ok(())
    }

    /// Deserialise a request header from the given stream.
    ///
    /// # Errors
    /// - if either the magic number or the header size are invalid values,
    /// `ResponseStatus::InvalidHeader` is returned.
    /// - if reading the fields after magic number and header size fails,
    /// `ResponseStatus::ConnectionError` is returned
    ///     - the read may fail due to a timeout if not enough bytes are
    ///     sent across
    /// - if the parsed bytes cannot be unmarshalled into the contained fields,
    /// `ResponseStatus::InvalidEncoding` is returned.
    /// - if the wire protocol version used is different than 1.0
    pub fn read_from_stream<R: Read>(mut stream: &mut R) -> Result<WireHeader> {
        let magic_number = get_from_stream!(stream, u32);
        if magic_number != MAGIC_NUMBER {
            error!(
                "Expected magic number {}, got {}",
                MAGIC_NUMBER, magic_number
            );
            return Err(ResponseStatus::InvalidHeader);
        }

        let hdr_size = get_from_stream!(stream, u16);
        let mut bytes = vec![0_u8; usize::try_from(hdr_size)?];
        stream.read_exact(&mut bytes)?;
        if hdr_size != REQUEST_HDR_SIZE {
            error!(
                "Expected request header size {}, got {}",
                REQUEST_HDR_SIZE, hdr_size
            );
            return Err(ResponseStatus::InvalidHeader);
        }

        let version_maj = bytes.remove(0); // first byte after hdr length is version maj
        let version_min = bytes.remove(0); // second byte after hdr length is version min
        if version_maj != WIRE_PROTOCOL_VERSION_MAJ || version_min != WIRE_PROTOCOL_VERSION_MIN {
            error!(
                "Expected wire protocol version {}.{}, got {}.{} instead",
                WIRE_PROTOCOL_VERSION_MAJ, WIRE_PROTOCOL_VERSION_MIN, version_maj, version_min
            );
            return Err(ResponseStatus::WireProtocolVersionNotSupported);
        }

        Ok(bincode::deserialize(&bytes)?)
    }
}
