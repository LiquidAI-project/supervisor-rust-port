#![forbid(unsafe_code)]
#![warn(clippy::dbg_macro)]

use crate::lib::error::Result;
use crate::lib::parser::Parser;
use crate::lib::source::BinarySource;
use crate::lib::wain_ast::Root;

pub fn parse(input: &[u8]) -> Result<'_, Root<'_, BinarySource<'_>>> {
    let mut parser = Parser::new(input);
    parser.parse()
}