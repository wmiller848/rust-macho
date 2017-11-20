use core::str;
use core::fmt;
use alloc::rc::Rc;
use nostd_io::{Cursor, Seek, SeekFrom};
use alloc::Vec;

use byteorder::{ByteOrder, ReadBytesExt, LittleEndian, BigEndian};

use errors::*;
use consts::*;
use commands::{LoadCommand, Section};
use loader::{OFile, MachCommand};

/// the link-edit 4.3BSD "stab" style symbol
#[derive(Debug)]
pub enum Symbol<'a> {
    Undefined {
        name: Option<&'a str>,
        external: bool,
        desc: u16,
    },
    Absolute {
        name: Option<&'a str>,
        external: bool,
        desc: u16,
        entry: usize,
    },
    Defined {
        name: Option<&'a str>,
        external: bool,
        section: Option<Rc<Section>>,
        desc: u16,
        entry: usize,
    },
    Prebound {
        name: Option<&'a str>,
        external: bool,
        desc: u16,
    },
    Indirect {
        name: Option<&'a str>,
        external: bool,
        desc: u16,
        symbol: Option<&'a str>,
    },
    Debug {
        name: Option<&'a str>,
        section: Option<Rc<Section>>,
        desc: u16,
        addr: usize,
    },
}

impl<'a> Symbol<'a> {
    pub fn name(&self) -> Option<&str> {
        match self {
            &Symbol::Undefined { name, .. } |
            &Symbol::Absolute { name, .. } |
            &Symbol::Defined { name, .. } |
            &Symbol::Prebound { name, .. } |
            &Symbol::Indirect { name, .. } |
            &Symbol::Debug { name, .. } => name,
        }
    }

    pub fn is_external(&self) -> bool {
        match self {
            &Symbol::Undefined { external, .. } |
            &Symbol::Absolute { external, .. } |
            &Symbol::Defined { external, .. } |
            &Symbol::Prebound { external, .. } |
            &Symbol::Indirect { external, .. } => external,
            _ => false,
        }
    }
}

impl<'a> fmt::Display for Symbol<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Symbol::Undefined { ref name, external, .. } => {
                write!(f,
                       "                 {} {}",
                       if external { "U" } else { "u" },
                       name.unwrap_or(""))
            }
            &Symbol::Absolute { ref name, external, entry, .. } => {
                write!(f,
                       "{:016x} {} {}",
                       entry,
                       if external { "A" } else { "a" },
                       name.unwrap_or(""))
            }
            &Symbol::Defined { ref name, external, ref section, entry, .. } => {
                let mut symtype = "s";

                if let &Some(ref section) = section {
                    let Section { ref sectname, ref segname, .. } = **section;

                    if segname == SEG_TEXT && sectname == SECT_TEXT {
                        symtype = "t"
                    } else if segname == SEG_DATA {
                        if sectname == SECT_DATA {
                            symtype = "d"
                        } else if sectname == SECT_BSS {
                            symtype = "b"
                        } else if sectname == SECT_COMMON {
                            symtype = "c"
                        }
                    }
                }

                write!(f,
                       "{:016x} {} {}",
                       entry,
                       if external {
                           symtype.to_uppercase()
                       } else {
                           symtype.to_lowercase()
                       },
                       name.unwrap_or(""))
            }
            &Symbol::Prebound { ref name, external, .. } => {
                write!(f,
                       "                 {} {}",
                       if external { "P" } else { "p" },
                       name.unwrap_or(""))
            }
            &Symbol::Indirect { ref name, external, .. } => {
                write!(f,
                       "                 {} {}",
                       if external { "I" } else { "i" },
                       name.unwrap_or(""))
            }
            &Symbol::Debug { ref name, addr, .. } => {
                if addr == 0 {
                    write!(f, "                 d {}", name.unwrap_or(""))
                } else {
                    write!(f, "{:016x} d {}", addr, name.unwrap_or(""))
                }
            }
        }
    }
}

/// Reference type and flags of symbol
pub trait SymbolReference {
    /// raw `desc` value
    fn desc(&self) -> u16;

    /// types of references
    fn ref_type(&self) -> u8 {
        self.desc() as u8 & REFERENCE_TYPE
    }

    /// To simplify stripping of objects that use are used with the dynamic link
    /// editor, the static link editor marks the symbols defined an object that are
    /// referenced by a dynamicly bound object (dynamic shared libraries, bundles).
    /// With this marking strip knows not to strip these symbols.
    fn is_ref_dyn(&self) -> bool {
        (self.desc() & REFERENCED_DYNAMICALLY) == REFERENCED_DYNAMICALLY
    }

    /// The ordinal recorded references the libraries listed in the Mach-O file
    fn lib_ordinal(&self) -> u8 {
        ((self.desc() >> 8) & 0xff) as u8
    }

    /// symbol is not to be dead stripped
    fn is_no_dead_strip(&self) -> bool {
        (self.desc() & N_NO_DEAD_STRIP) == N_NO_DEAD_STRIP
    }

    /// symbol is discarded
    fn is_discarded(&self) -> bool {
        (self.desc() & N_DESC_DISCARDED) == N_DESC_DISCARDED
    }

    /// symbol is weak referenced
    fn is_weak_ref(&self) -> bool {
        (self.desc() & N_WEAK_REF) == N_WEAK_REF
    }

    /// coalesed symbol is a weak definition
    fn is_weak_def(&self) -> bool {
        (self.desc() & N_WEAK_DEF) == N_WEAK_DEF
    }

    /// reference to a weak symbol
    fn is_ref_to_weak(&self) -> bool {
        (self.desc() & N_REF_TO_WEAK) == N_REF_TO_WEAK
    }

    /// symbol is a Thumb function (ARM)
    fn is_arm_thumb_def(&self) -> bool {
        (self.desc() & N_ARM_THUMB_DEF) == N_ARM_THUMB_DEF
    }

    /// the function is actually a resolver function and
    /// should be called to get the address of the real function to use.
    fn is_resolver(&self) -> bool {
        (self.desc() & N_SYMBOL_RESOLVER) == N_SYMBOL_RESOLVER
    }

    /// symbol is pinned to the previous content.
    fn is_alt_entry(&self) -> bool {
        (self.desc() & N_ALT_ENTRY) == N_ALT_ENTRY
    }
}

impl<'a> SymbolReference for Symbol<'a> {
    fn desc(&self) -> u16 {
        match self {
            &Symbol::Undefined { desc, .. } |
            &Symbol::Absolute { desc, .. } |
            &Symbol::Defined { desc, .. } |
            &Symbol::Prebound { desc, .. } |
            &Symbol::Indirect { desc, .. } |
            &Symbol::Debug { desc, .. } => desc,
        }
    }
}

/// `Symbol` Iter
pub struct SymbolIter<'a> {
    cur: &'a mut Cursor<&'a [u8]>,
    sections: Vec<Rc<Section>>,
    nsyms: u32,
    stroff: u32,
    strsize: u32,
    is_bigend: bool,
    is_64bit: bool,
}

impl<'a> SymbolIter<'a> {
    fn parse(&mut self) -> Result<Symbol<'a>> {
        if self.is_bigend {
            self.parse_symbol::<BigEndian>()
        } else {
            self.parse_symbol::<LittleEndian>()
        }
    }

    pub fn parse_symbol<O: ByteOrder>(&mut self) -> Result<Symbol<'a>> {
        let strx = try!(self.cur.read_u32::<O>()) as usize;
        let flags = try!(self.cur.read_u8());
        let sect = try!(self.cur.read_u8());
        let desc = try!(self.cur.read_u16::<O>());
        let value = if self.is_64bit {
            try!(self.cur.read_u64::<O>()) as usize
        } else {
            try!(self.cur.read_u32::<O>()) as usize
        };

        if (flags & N_STAB) != 0 {
            Ok(Symbol::Debug {
                name: try!(self.load_str(strx)),
                section: if sect == NO_SECT {
                    None
                } else {
                    self.sections.get((sect - 1) as usize).map(|x| x.clone())
                },
                desc: desc,
                addr: value,
            })
        } else {
            let external = (flags & N_EXT) == N_EXT;

            let typ = flags & N_TYPE;

            match typ {
                N_UNDF => {
                    Ok(Symbol::Undefined {
                        name: try!(self.load_str(strx)),
                        external: external,
                        desc: desc,
                    })
                }
                N_ABS => {
                    Ok(Symbol::Absolute {
                        name: try!(self.load_str(strx)),
                        external: external,
                        desc: desc,
                        entry: value,
                    })
                }
                N_SECT => {
                    Ok(Symbol::Defined {
                        name: try!(self.load_str(strx)),
                        external: external,
                        section: if sect == NO_SECT {
                            None
                        } else {
                            self.sections.get((sect - 1) as usize).map(|x| x.clone())
                        },
                        desc: desc,
                        entry: value,
                    })
                }
                N_PBUD => {
                    Ok(Symbol::Prebound {
                        name: try!(self.load_str(strx)),
                        external: external,
                        desc: desc,
                    })
                }
                N_INDR => {
                    Ok(Symbol::Indirect {
                        name: try!(self.load_str(strx)),
                        external: external,
                        desc: desc,
                        symbol: try!(self.load_str(value)),
                    })
                }
                _ => Err(Error::LoadError(format!("unknown symbol type 0x{:x}", typ))),
            }
        }
    }

    fn load_str(&mut self, off: usize) -> Result<Option<&'a str>> {
        if off == 0 {
            Ok(None)
        } else if off >= self.strsize as usize {
            Err(Error::LoadError(format!("string offset out of range [..{})", self.strsize)))
        } else {
            let buf = *self.cur.get_ref();
            let s = *&buf[self.stroff as usize + off as usize..]
                .split(|x| *x == 0)
                .next()
                .unwrap();

            Ok(Some(try!(str::from_utf8(s))))
        }
    }
}

impl<'a> Iterator for SymbolIter<'a> {
    type Item = Symbol<'a>;

    fn next(&mut self) -> Option<Symbol<'a>> {
        if self.nsyms > 0 {
            if let Ok(symbol) = self.parse() {
                self.nsyms -= 1;

                return Some(symbol);
            }
        }

        None
    }
}

/// Read symbols from a Mach-O file
pub trait SymbolReader<'a> {
    type Iter: Iterator<Item = Symbol<'a>>;

    /// Read symbols from Mach-O file
    fn symbols(&self, cur: &'a mut Cursor<&'a [u8]>) -> Option<Self::Iter>;
}

impl<'a> SymbolReader<'a> for OFile {
    type Iter = SymbolIter<'a>;

    fn symbols(&self, cur: &'a mut Cursor<&'a [u8]>) -> Option<Self::Iter> {
        if let &OFile::MachFile { ref header, ref commands } = self {
            let sections = commands.iter()
                .filter_map(|cmd| match cmd.0 {
                    LoadCommand::Segment { ref sections, .. } |
                    LoadCommand::Segment64 { ref sections, .. } => Some(sections),
                    _ => None,
                })
                .flat_map(|sections| sections.clone())
                .collect();

            for cmd in commands {
                let &MachCommand(ref cmd, _) = cmd;

                if let &LoadCommand::SymTab { symoff, nsyms, stroff, strsize } = cmd {
                    if let Ok(_) = cur.seek(SeekFrom::Start(symoff as u64)) {
                        return Some(SymbolIter {
                            cur: cur,
                            sections: sections,
                            nsyms: nsyms,
                            stroff: stroff,
                            strsize: strsize,
                            is_bigend: header.is_bigend(),
                            is_64bit: header.is_64bit(),
                        });
                    }
                }
            }
        }

        None
    }
}

// The n_type field really contains four fields:
//  unsigned char N_STAB:3,
//            N_PEXT:1,
//            N_TYPE:3,
//            N_EXT:1;
// which are used via the following masks.
//
const N_STAB: u8 = 0xe0;  /* if any of these bits set, a symbolic debugging entry */
#[allow(dead_code)]
const N_PEXT: u8 = 0x10;  /* private external symbol bit */
const N_TYPE: u8 = 0x0e;  /* mask for the type bits */
const N_EXT: u8 = 0x01;  /* external symbol bit, set for external symbols */



// Values for N_TYPE bits of the n_type field.
//
const N_UNDF: u8 = 0x0;    /* undefined, n_sect == NO_SECT */
const N_ABS: u8 = 0x2;    /* absolute, n_sect == NO_SECT */
const N_SECT: u8 = 0xe;    /* defined in section number n_sect */
const N_PBUD: u8 = 0xc;    /* prebound undefined (defined in a dylib) */
const N_INDR: u8 = 0xa;    /* indirect */

const NO_SECT: u8 = 0;   /* symbol is not in any section */
