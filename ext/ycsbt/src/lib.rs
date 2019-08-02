/* Copyright (c) 2018 University of Utah
 *
 * Permission to use, copy, modify, and distribute this software for any
 * purpose with or without fee is hereby granted, provided that the above
 * copyright notice and this permission notice appear in all copies.
 *
 * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR(S) DISCLAIM ALL WARRANTIES
 * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL AUTHORS BE LIABLE FOR
 * ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
 * ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
 * OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
 */

#![crate_type = "dylib"]
#![forbid(unsafe_code)]
#![feature(generators, generator_trait)]

extern crate sandstorm;

use std::ops::Generator;
use std::rc::Rc;

use sandstorm::db::DB;

/// This function implements the get() extension using the sandstorm interface.
///
/// # Arguments
///
/// * `db`: An argument whose type implements the `DB` trait which can be used
///         to interact with the database.
///
/// # Return
///
/// A coroutine that can be run inside the database.
#[no_mangle]
#[allow(unreachable_code)]
#[allow(unused_assignments)]
pub fn init(db: Rc<DB>) -> Box<Generator<Yield = u64, Return = u64>> {
    Box::new(move || {
        let key_len = 30;
        let mut table: u64 = 0;
        let mut optype = 0;
        let mut keys = Vec::with_capacity(2 * key_len);
        let mut obj = None;
        let mut multiobj = None;
        {
            // First off, retrieve the arguments to the extension.
            let args = db.args();

            // Check that the arguments received is long enough to contain an 1 bytes
            // operation type, 8 byte table id and a key to be looked up. If not, then
            // write an error message to the response and return to the database.
            if args.len() <= 38 {
                let error = "Invalid args";
                db.resp(error.as_bytes());
                return 1;
            }

            let (op, rest) = args.split_at(1);
            optype = op[0];

            // Next, split the arguments into a view over the table identifier
            // (first eight bytes), and a view over the key to be looked up.
            // De-serialize the table identifier into a u64.
            let (s_table, key) = rest.split_at(8);
            keys.extend_from_slice(key);

            // Get the table id from the unwrapped arguments.
            for (idx, e) in s_table.iter().enumerate() {
                table |= (*e as u64) << (idx << 3);
            }

            if optype == 1 {
                obj = db.get(table, key);
            } else {
                multiobj = db.multiget(table, key_len as u16, &keys);
            }
        }

        if optype == 1 {
            // Read operation
            match obj {
                Some(val) => {
                    db.resp(val.read());
                    return 0;
                }

                None => {
                    let error = "Object does not exist";
                    db.resp(error.as_bytes());
                    return 1;
                }
            }
        } else {
            // Read-modify-write operation
            match multiobj {
                Some(vals) => {
                    if vals.num() == 2 {
                        let mut value1 = Vec::with_capacity(vals.len() / 2);
                        let mut value2 = Vec::with_capacity(vals.len() / 2);
                        let (key1, key2) = keys.split_at(key_len as usize);
                        value1.extend_from_slice(vals.read());
                        let _ = vals.next();
                        value2.extend_from_slice(vals.read());
                        if value1[0] > 0 {
                            value1[0] -= 1;
                            value2[0] += 1;
                        } else if value2[0] > 0 {
                            value2[0] -= 1;
                            value1[0] += 1;
                        }

                        if let Some(mut buf1) = db.alloc(table, key1, value1.len() as u64) {
                            if let Some(mut buf2) = db.alloc(table, key2, value2.len() as u64) {
                                buf1.write_slice(&value1);
                                buf2.write_slice(&value2);
                                db.put(buf1);
                                db.put(buf2);
                                return 0;
                            }
                        }
                    }

                    let error = "Error";
                    db.resp(error.as_bytes());
                    return 1;
                }

                None => {
                    let error = "Object does not exist";
                    db.resp(error.as_bytes());
                    return 1;
                }
            }
        }

        // XXX: This yield is required to get the compiler to compile this closure into a
        // generator. It is unreachable and benign.
        yield 0;
    })
}