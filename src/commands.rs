use core::fmt;
use core::str;
use alloc::rc::Rc;
use nostd_io::{Read, BufRead, Cursor};
use alloc::Vec;
use alloc::String;

use uuid::Uuid;
use byteorder::{ByteOrder, ReadBytesExt};

use consts::*;
use errors::*;

/// The encoded version.
///
///  X.Y.Z is encoded in nibbles xxxx.yy.zz
///
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct VersionTag(u32);

impl VersionTag {
    pub fn major(self) -> u32 {
        self.0 >> 16
    }

    pub fn minor(self) -> u32 {
        (self.0 >> 8) & 0xFF
    }

    pub fn release(self) -> u32 {
        self.0 & 0xFF
    }
}

impl Into<u32> for VersionTag {
    fn into(self) -> u32 {
        self.0
    }
}

impl fmt::Display for VersionTag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.release() == 0 {
            write!(f, "{}.{}", self.major(), self.minor())
        } else {
            write!(f, "{}.{}.{}", self.major(), self.minor(), self.release())
        }
    }
}

/// The packed version.
///
/// A.B.C.D.E packed as a24.b10.c10.d10.e10
///
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct SourceVersionTag(u64);

impl Into<u64> for SourceVersionTag {
    fn into(self) -> u64 {
        self.0
    }
}

impl Into<(u32, u32, u32, u32, u32)> for SourceVersionTag {
    fn into(self) -> (u32, u32, u32, u32, u32) {
        (((self.0 >> 40) & 0xFFF) as u32,
         ((self.0 >> 30) & 0x3FF) as u32,
         ((self.0 >> 20) & 0x3FF) as u32,
         ((self.0 >> 10) & 0x3FF) as u32,
         (self.0 & 0x3FF) as u32)
    }
}

impl fmt::Display for SourceVersionTag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (a, b, c, d, e) = Self::into(*self);

        if e != 0 {
            write!(f, "{}.{}.{}.{}.{}", a, b, c, d, e)
        } else if d != 0 {
            write!(f, "{}.{}.{}.{}", a, b, c, d)
        } else if c != 0 {
            write!(f, "{}.{}.{}", a, b, c)
        } else {
            write!(f, "{}.{}", a, b)
        }
    }
}

/// The min OS version on which this binary was built to run.
///
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BuildTarget {
    MacOsX,
    IPhoneOs,
    WatchOs,
    TvOs,
}

impl From<u32> for BuildTarget {
    fn from(cmd: u32) -> Self {
        match cmd {
            LC_VERSION_MIN_MACOSX => BuildTarget::MacOsX,
            LC_VERSION_MIN_IPHONEOS => BuildTarget::IPhoneOs,
            LC_VERSION_MIN_WATCHOS => BuildTarget::WatchOs,
            LC_VERSION_MIN_TVOS => BuildTarget::TvOs,
            _ => unreachable!(),
        }
    }
}

impl Into<u32> for BuildTarget {
    fn into(self) -> u32 {
        match self {
            BuildTarget::MacOsX => LC_VERSION_MIN_MACOSX,
            BuildTarget::IPhoneOs => LC_VERSION_MIN_IPHONEOS,
            BuildTarget::WatchOs => LC_VERSION_MIN_WATCHOS,
            BuildTarget::TvOs => LC_VERSION_MIN_TVOS,
        }
    }
}

/// A variable length string in a load command is represented by an LcString structure.
///
/// The strings are stored just after the load command structure and
/// the offset is from the start of the load command structure.  The size
/// of the string is reflected in the cmdsize field of the load command.
/// Once again any padded bytes to bring the cmdsize field to a multiple
/// of 4 bytes must be zero.
///
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LcString(pub usize, pub String);

impl fmt::Display for LcString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.1)
    }
}

/// Fixed virtual memory shared libraries are identified by two things.
///
/// The target pathname (the name of the library as found for execution),
/// and the minor version number.  The address of where the headers are loaded is in
/// header_addr. (THIS IS OBSOLETE and no longer supported).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FvmLib {
    /// library's target pathname
    pub name: LcString,
    /// library's minor version number
    pub minor_version: u32,
    /// library's header address
    pub header_addr: u32,
}


/// Dynamically linked shared libraries are identified by two things.
///
/// The pathname (the name of the library as found for execution), and the
/// compatibility version number.  The pathname must match and the compatibility
/// number in the user of the library must be greater than or equal to the
/// library being used.  The time stamp is used to record the time a library was
/// built and copied into user so it can be use to determined if the library used
/// at runtime is exactly the same as used to built the program.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DyLib {
    /// library's path name
    pub name: LcString,
    /// library's build time stamp
    pub timestamp: u32,
    /// library's current version number
    pub current_version: VersionTag,
    /// library's compatibility vers number
    pub compatibility_version: VersionTag,
}

/// a table of contents entry
pub struct DyLibTocEntry {
    /// the defined external symbol (index into the symbol table)
    pub symbol_index: u32,
    /// index into the module table this symbol is defined in
    pub module_index: u32,
}

/// a module table entry
pub struct DyLibModule {
    /// the module name (index into string table)
    pub module_name: u32,

    /// index into externally defined symbols
    pub iextdefsym: u32,
    /// number of externally defined symbols
    pub nextdefsym: u32,
    /// index into reference symbol table
    pub irefsym: u32,
    /// number of reference symbol table entries
    pub nrefsym: u32,
    /// index into symbols for local symbols
    pub ilocalsym: u32,
    /// number of local symbols
    pub nlocalsym: u32,

    /// index into external relocation entries
    pub iextrel: u32,
    /// number of external relocation entries
    pub nextrel: u32,

    /// low 16 bits are the index into the init section,
    /// high 16 bits are the index into the term section
    pub iinit_iterm: u32,
    /// low 16 bits are the number of init section entries,
    /// high 16 bits are the number of term section entries
    pub ninit_nterm: u32,

    /// for this module address of the start of the (__OBJC,__module_info) section
    pub objc_module_info_addr: u32,
    /// for this module size of the (__OBJC,__module_info) section
    pub objc_module_info_size: usize,
}

/// The linkedit_data_command contains the offsets and sizes of a blob
/// of data in the __LINKEDIT segment.
///
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LinkEditData {
    /// file offset of data in __LINKEDIT segment
    pub off: u32,
    /// file size of data in __LINKEDIT segment
    pub size: u32,
}

/// The load commands directly follow the mach_header.
///
#[derive(Debug, Clone)]
pub enum LoadCommand {
    /// The segment load command indicates that a part of this file is to be
    /// mapped into the task's address space.
    ///
    /// The size of this segment in memory, vmsize, maybe equal to or
    /// larger than the amount to map from this file, filesize.
    /// The file is mapped starting at fileoff to the beginning of
    /// the segment in memory, vmaddr.  The rest of the memory of the segment,
    /// if any, is allocated zero fill on demand.  The segment's maximum virtual
    /// memory protection and initial virtual memory protection are specified
    /// by the maxprot and initprot fields.  If the segment has sections then the
    /// section structures directly follow the segment command and their size is
    /// reflected in cmdsize.
    ///
    Segment {
        /// segment name
        segname: String,
        /// memory address of this segment
        vmaddr: usize,
        /// memory size of this segment
        vmsize: usize,
        /// file offset of this segment
        fileoff: usize,
        /// amount to map from the file
        filesize: usize,
        /// maximum VM protection
        maxprot: vm_prot_t,
        /// initial VM protection
        initprot: vm_prot_t,
        /// flags
        flags: SegmentFlags,
        /// sections
        sections: Vec<Rc<Section>>,
    },
    /// The 64-bit segment load command indicates that a part of this file is to be
    /// mapped into a 64-bit task's address space.
    ///
    /// If the 64-bit segment has sections then section_64 structures directly follow
    /// the 64-bit segment command and their size is reflected in cmdsize.
    ///
    Segment64 {
        /// segment name
        segname: String,
        /// memory address of this segment
        vmaddr: usize,
        /// memory size of this segment
        vmsize: usize,
        /// file offset of this segment
        fileoff: usize,
        /// amount to map from the file
        filesize: usize,
        /// maximum VM protection
        maxprot: vm_prot_t,
        /// initial VM protection
        initprot: vm_prot_t,
        /// flags
        flags: SegmentFlags,
        /// sections
        sections: Vec<Rc<Section>>,
    },

    // A fixed virtual shared library (filetype == MH_FVMLIB in the mach header)
    // contains a fvmlib_command (cmd == LC_IDFVMLIB) to identify the library.
    //
    // An object that uses a fixed virtual shared library also contains a
    // fvmlib_command (cmd == LC_LOADFVMLIB) for each library it uses.
    // (THIS IS OBSOLETE and no longer supported).
    //
    /// fixed VM shared library identification
    IdFvmLib(FvmLib),
    /// load a specified fixed VM shared library
    LoadFvmLib(FvmLib),

    // A dynamically linked shared library (filetype == MH_DYLIB in the mach header)
    // contains a dylib_command (cmd == LC_ID_DYLIB) to identify the library.
    //
    // An object that uses a dynamically linked shared library also contains a
    // dylib_command (cmd == LC_LOAD_DYLIB, LC_LOAD_WEAK_DYLIB, or
    // LC_REEXPORT_DYLIB) for each library it uses.
    //
    /// dynamically linked shared lib ident
    IdDyLib(DyLib),
    /// load a dynamically linked shared library
    LoadDyLib(DyLib),
    /// load a dynamically linked shared library
    /// that is allowed to be missing (all symbols are weak imported).
    LoadWeakDyLib(DyLib),
    /// load and re-export dylib
    ReexportDyLib(DyLib),
    /// load upward dylib
    LoadUpwardDylib(DyLib),
    /// delay load of dylib until first use
    LazyLoadDylib(DyLib),

    // A program that uses a dynamic linker contains a dylinker_command to identify
    // the name of the dynamic linker (LC_LOAD_DYLINKER).  And a dynamic linker
    // contains a dylinker_command to identify the dynamic linker (LC_ID_DYLINKER).
    // A file can have at most one of these.
    // This struct is also used for the LC_DYLD_ENVIRONMENT load command and
    // contains string for dyld to treat like environment variable.
    //
    /// dynamic linker identification
    IdDyLinker(LcString),
    /// load a dynamic linker
    LoadDyLinker(LcString),
    /// string for dyld to treat like environment variable
    DyLdEnv(LcString),

    /// The symtab_command contains the offsets and sizes of the link-edit 4.3BSD
    /// "stab" style symbol table information as described in the header files
    /// <nlist.h> and <stab.h>.
    ///
    SymTab {
        /// symbol table offset
        symoff: u32,
        /// number of symbol table entries
        nsyms: u32,
        /// string table offset
        stroff: u32,
        /// string table size in bytes
        strsize: u32,
    },

    /// This is the second set of the symbolic information which is used to support
    /// the data structures for the dynamically link editor.
    ///
    /// The original set of symbolic information in the symtab_command which contains
    /// the symbol and string tables must also be present when this load command is
    /// present.  When this load command is present the symbol table is organized
    /// into three groups of symbols:
    ///  local symbols (static and debugging symbols) - grouped by module
    ///  defined external symbols - grouped by module (sorted by name if not lib)
    ///  undefined external symbols (sorted by name if MH_BINDATLOAD is not set,
    ///                      and in order the were seen by the static
    ///                  linker if MH_BINDATLOAD is set)
    /// In this load command there are offsets and counts to each of the three groups
    /// of symbols.
    ///
    /// This load command contains a the offsets and sizes of the following new
    /// symbolic information tables:
    ///  table of contents
    ///  module table
    ///  reference symbol table
    ///  indirect symbol table
    /// The first three tables above (the table of contents, module table and
    /// reference symbol table) are only present if the file is a dynamically linked
    /// shared library.  For executable and object modules, which are files
    /// containing only one module, the information that would be in these three
    /// tables is determined as follows:
    ///  table of contents - the defined external symbols are sorted by name
    ///  module table - the file contains only one module so everything in the
    ///             file is part of the module.
    ///  reference symbol table - is the defined and undefined external symbols
    ///
    /// For dynamically linked shared library files this load command also contains
    /// offsets and sizes to the pool of relocation entries for all sections
    /// separated into two groups:
    ///  external relocation entries
    ///  local relocation entries
    /// For executable and object modules the relocation entries continue to hang
    /// off the section structures.
    ///
    DySymTab {
        // The symbols indicated by symoff and nsyms of the LC_SYMTAB load command
        // are grouped into the following three groups:
        //    local symbols (further grouped by the module they are from)
        //    defined external symbols (further grouped by the module they are from)
        //    undefined symbols
        //
        // The local symbols are used only for debugging.  The dynamic binding
        // process may have to use them to indicate to the debugger the local
        // symbols for a module that is being bound.
        //
        // The last two groups are used by the dynamic binding process to do the
        // binding (indirectly through the module table and the reference symbol
        // table when this is a dynamically linked shared library file).
        //
        /// index to local symbols
        ilocalsym: u32,
        /// number of local symbols
        nlocalsym: u32,

        /// index to externally defined symbols
        iextdefsym: u32,
        /// number of externally defined symbols
        nextdefsym: u32,

        /// index to undefined symbols
        iundefsym: u32,
        /// number of undefined symbols
        nundefsym: u32,

        // For the for the dynamic binding process to find which module a symbol
        // is defined in the table of contents is used (analogous to the ranlib
        // structure in an archive) which maps defined external symbols to modules
        // they are defined in.  This exists only in a dynamically linked shared
        // library file.  For executable and object modules the defined external
        // symbols are sorted by name and is use as the table of contents.
        //
        /// file offset to table of contents
        tocoff: u32,
        /// number of entries in table of contents
        ntoc: u32,

        // To support dynamic binding of "modules" (whole object files) the symbol
        // table must reflect the modules that the file was created from.  This is
        // done by having a module table that has indexes and counts into the merged
        // tables for each module.  The module structure that these two entries
        // refer to is described below.  This exists only in a dynamically linked
        // shared library file.  For executable and object modules the file only
        // contains one module so everything in the file belongs to the module.
        //
        /// file offset to module table
        modtaboff: u32,
        /// number of module table entries
        nmodtab: u32,

        // To support dynamic module binding the module structure for each module
        // indicates the external references (defined and undefined) each module
        // makes.  For each module there is an offset and a count into the
        // reference symbol table for the symbols that the module references.
        // This exists only in a dynamically linked shared library file.  For
        // executable and object modules the defined external symbols and the
        // undefined external symbols indicates the external references.
        //
        /// offset to referenced symbol table
        extrefsymoff: u32,
        /// number of referenced symbol table entries
        nextrefsyms: u32,

        // The sections that contain "symbol pointers" and "routine stubs" have
        // indexes and (implied counts based on the size of the section and fixed
        // size of the entry) into the "indirect symbol" table for each pointer
        // and stub.  For every section of these two types the index into the
        // indirect symbol table is stored in the section header in the field
        // reserved1.  An indirect symbol table entry is simply a 32bit index into
        // the symbol table to the symbol that the pointer or stub is referring to.
        // The indirect symbol table is ordered to match the entries in the section.
        //
        /// file offset to the indirect symbol table
        indirectsymoff: u32,
        /// number of indirect symbol table entries
        nindirectsyms: u32,

        // To support relocating an individual module in a library file quickly the
        // external relocation entries for each module in the library need to be
        // accessed efficiently.  Since the relocation entries can't be accessed
        // through the section headers for a library file they are separated into
        // groups of local and external entries further grouped by module.  In this
        // case the presents of this load command who's extreloff, nextrel,
        // locreloff and nlocrel fields are non-zero indicates that the relocation
        // entries of non-merged sections are not referenced through the section
        // structures (and the reloff and nreloc fields in the section headers are
        // set to zero).
        //
        // Since the relocation entries are not accessed through the section headers
        // this requires the r_address field to be something other than a section
        // offset to identify the item to be relocated.  In this case r_address is
        // set to the offset from the vmaddr of the first LC_SEGMENT command.
        // For MH_SPLIT_SEGS images r_address is set to the the offset from the
        // vmaddr of the first read-write LC_SEGMENT command.
        //
        // The relocation entries are grouped by module and the module table
        // entries have indexes and counts into them for the group of external
        // relocation entries for that the module.
        //
        // For sections that are merged across modules there must not be any
        // remaining external relocation entries for them (for merged sections
        // remaining relocation entries must be local).
        //
        /// offset to external relocation entries
        extreloff: u32,
        /// number of external relocation entries
        nextrel: u32,

        // All the local relocation entries are grouped together (they are not
        // grouped by their module since they are only used if the object is moved
        // from it staticly link edited address).
        //
        /// offset to local relocation entries
        locreloff: u32,
        /// number of local relocation entries
        nlocrel: u32,
    },

    /// The uuid load command contains a single 128-bit unique random number that
    /// identifies an object produced by the static link editor.
    ///
    Uuid(Uuid),

    // The linkedit_data_command contains the offsets and sizes of a blob
    // of data in the __LINKEDIT segment.
    //
    /// local of code signature
    CodeSignature(LinkEditData),
    /// local of info to split segments
    SegmentSplitInfo(LinkEditData),
    /// compressed table of function start addresses
    FunctionStarts(LinkEditData),
    /// table of non-instructions in __text
    DataInCode(LinkEditData),
    /// Code signing DRs copied from linked dylibs
    DylibCodeSignDrs(LinkEditData),
    /// optimization hints in MH_OBJECT files
    LinkerOptimizationHint(LinkEditData),

    /// The version_min_command contains the min OS version on which this
    /// binary was built to run.
    ///
    VersionMin {
        target: BuildTarget,
        version: VersionTag,
        sdk: VersionTag,
    },

    /// The dyld_info_command contains the file offsets and sizes of
    /// the new compressed form of the information dyld needs to
    /// load the image.  This information is used by dyld on Mac OS X
    /// 10.6 and later.  All information pointed to by this command
    /// is encoded using byte streams, so no endian swapping is needed
    /// to interpret it.
    ///
    DyldInfo {
        // Dyld rebases an image whenever dyld loads it at an address different
        // from its preferred address.  The rebase information is a stream
        // of byte sized opcodes whose symbolic names start with REBASE_OPCODE_.
        // Conceptually the rebase information is a table of tuples:
        //    <seg-index, seg-offset, type>
        // The opcodes are a compressed way to encode the table by only
        // encoding when a column changes.  In addition simple patterns
        // like "every n'th offset for m times" can be encoded in a few
        // bytes.
        //
        /// file offset to rebase info
        rebase_off: u32,
        /// size of rebase info
        rebase_size: u32,

        // Dyld binds an image during the loading process, if the image
        // requires any pointers to be initialized to symbols in other images.
        // The bind information is a stream of byte sized
        // opcodes whose symbolic names start with BIND_OPCODE_.
        // Conceptually the bind information is a table of tuples:
        //    <seg-index, seg-offset, type, symbol-library-ordinal, symbol-name, addend>
        // The opcodes are a compressed way to encode the table by only
        // encoding when a column changes.  In addition simple patterns
        // like for runs of pointers initialzed to the same value can be
        // encoded in a few bytes.
        //
        /// file offset to binding info
        bind_off: u32,
        /// size of binding info
        bind_size: u32,

        // Some C++ programs require dyld to unique symbols so that all
        // images in the process use the same copy of some code/data.
        // This step is done after binding. The content of the weak_bind
        // info is an opcode stream like the bind_info.  But it is sorted
        // alphabetically by symbol name.  This enable dyld to walk
        // all images with weak binding information in order and look
        // for collisions.  If there are no collisions, dyld does
        // no updating.  That means that some fixups are also encoded
        // in the bind_info.  For instance, all calls to "operator new"
        // are first bound to libstdc++.dylib using the information
        // in bind_info.  Then if some image overrides operator new
        // that is detected when the weak_bind information is processed
        // and the call to operator new is then rebound.
        //
        /// file offset to weak binding info
        weak_bind_off: u32,
        /// size of weak binding info
        weak_bind_size: u32,

        // Some uses of external symbols do not need to be bound immediately.
        // Instead they can be lazily bound on first use.  The lazy_bind
        // are contains a stream of BIND opcodes to bind all lazy symbols.
        // Normal use is that dyld ignores the lazy_bind section when
        // loading an image.  Instead the static linker arranged for the
        // lazy pointer to initially point to a helper function which
        // pushes the offset into the lazy_bind area for the symbol
        // needing to be bound, then jumps to dyld which simply adds
        // the offset to lazy_bind_off to get the information on what
        // to bind.
        //
        /// file offset to lazy binding info
        lazy_bind_off: u32,
        /// size of lazy binding infs
        lazy_bind_size: u32,

        // The symbols exported by a dylib are encoded in a trie.  This
        // is a compact representation that factors out common prefixes.
        // It also reduces LINKEDIT pages in RAM because it encodes all
        // information (name, address, flags) in one small, contiguous range.
        // The export area is a stream of nodes.  The first node sequentially
        // is the start node for the trie.
        //
        // Nodes for a symbol start with a uleb128 that is the length of
        // the exported symbol information for the string so far.
        // If there is no exported symbol, the node starts with a zero byte.
        // If there is exported info, it follows the length.
        //
        // First is a uleb128 containing flags. Normally, it is followed by
        // a uleb128 encoded offset which is location of the content named
        // by the symbol from the mach_header for the image.  If the flags
        // is EXPORT_SYMBOL_FLAGS_REEXPORT, then following the flags is
        // a uleb128 encoded library ordinal, then a zero terminated
        // UTF8 string.  If the string is zero length, then the symbol
        // is re-export from the specified dylib with the same name.
        // If the flags is EXPORT_SYMBOL_FLAGS_STUB_AND_RESOLVER, then following
        // the flags is two uleb128s: the stub offset and the resolver offset.
        // The stub is used by non-lazy pointers.  The resolver is used
        // by lazy pointers and must be called to get the actual address to use.
        //
        // After the optional exported symbol information is a byte of
        // how many edges (0-255) that this node has leaving it,
        // followed by each edge.
        // Each edge is a zero terminated UTF8 of the addition chars
        // in the symbol, followed by a uleb128 offset for the node that
        // edge points to.
        //
        //
        /// file offset to lazy binding info
        export_off: u32,
        /// size of lazy binding infs
        export_size: u32,
    },

    /// The entry_point_command is a replacement for thread_command.
    /// It is used for main executables to specify the location (file offset)
    /// of main().  If -stack_size was used at link time, the stacksize
    /// field will contain the stack size need for the main thread.
    ///
    EntryPoint {
        /// file (__TEXT) offset of main()
        entryoff: u64,
        /// if not zero, initial stack size
        stacksize: u64,
    },
    /// The source_version_command is an optional load command containing
    /// the version of the sources used to build the binary.
    ///
    SourceVersion(SourceVersionTag),
    Command {
        /// type of load command
        cmd: u32,
        ///
        /// command in bytes
        payload: Vec<u8>,
    },
}

/// Read a fixed size string
pub trait ReadStringExt: Read {
    /// Read the fixed size string
    fn read_fixed_size_string(&mut self, len: usize) -> Result<String> {
        let mut buf = Vec::new();

        buf.resize(len + 1, 0);
        buf.truncate(len);

        try!(self.read_exact(buf.as_mut()));

        let len = buf.iter().position(|c| *c == 0x00).unwrap_or(buf.len());
        Ok(String::from(try!(str::from_utf8(&buf[..len]))))
    }
}

impl<R: Read + ?Sized> ReadStringExt for R {}

impl LoadCommand {
    pub fn parse<O: ByteOrder, T: AsRef<[u8]>>(buf: &mut Cursor<T>) -> Result<(LoadCommand, usize)> {
        let begin = buf.position();
        let cmd = try!(buf.read_u32::<O>());
        let cmdsize = try!(buf.read_u32::<O>());

        let cmd = match cmd {
            LC_SEGMENT => {
                let segname = try!(buf.read_fixed_size_string(16));
                let vmaddr = try!(buf.read_u32::<O>()) as usize;
                let vmsize = try!(buf.read_u32::<O>()) as usize;
                let fileoff = try!(buf.read_u32::<O>()) as usize;
                let filesize = try!(buf.read_u32::<O>()) as usize;
                let maxprot = try!(buf.read_i32::<O>());
                let initprot = try!(buf.read_i32::<O>());
                let nsects = try!(buf.read_u32::<O>());
                let flags = try!(buf.read_u32::<O>());
                let mut sections = Vec::new();

                for _ in 0..nsects {
                    sections.push(Rc::new(try!(Section::parse_section::<Cursor<T>, O>(buf))));
                }

                LoadCommand::Segment {
                    segname: segname,
                    vmaddr: vmaddr,
                    vmsize: vmsize,
                    fileoff: fileoff,
                    filesize: filesize,
                    maxprot: maxprot,
                    initprot: initprot,
                    flags: SegmentFlags::from_bits_truncate(flags),
                    sections: sections,
                }
            }
            LC_SEGMENT_64 => {
                let segname = try!(buf.read_fixed_size_string(16));
                let vmaddr = try!(buf.read_u64::<O>()) as usize;
                let vmsize = try!(buf.read_u64::<O>()) as usize;
                let fileoff = try!(buf.read_u64::<O>()) as usize;
                let filesize = try!(buf.read_u64::<O>()) as usize;
                let maxprot = try!(buf.read_i32::<O>());
                let initprot = try!(buf.read_i32::<O>());
                let nsects = try!(buf.read_u32::<O>());
                let flags = try!(buf.read_u32::<O>());
                let mut sections = Vec::new();

                for _ in 0..nsects {
                    sections.push(Rc::new(try!(Section::parse_section64::<Cursor<T>, O>(buf))));
                }

                LoadCommand::Segment64 {
                    segname: segname,
                    vmaddr: vmaddr,
                    vmsize: vmsize,
                    fileoff: fileoff,
                    filesize: filesize,
                    maxprot: maxprot,
                    initprot: initprot,
                    flags: SegmentFlags::from_bits_truncate(flags),
                    sections: sections,
                }
            }
            LC_IDFVMLIB => LoadCommand::IdFvmLib(try!(Self::read_fvmlib::<O, T>(buf))),
            LC_LOADFVMLIB => LoadCommand::LoadFvmLib(try!(Self::read_fvmlib::<O, T>(buf))),

            LC_ID_DYLIB => LoadCommand::IdDyLib(try!(Self::read_dylib::<O, T>(buf))),
            LC_LOAD_DYLIB => LoadCommand::LoadDyLib(try!(Self::read_dylib::<O, T>(buf))),
            LC_LOAD_WEAK_DYLIB => LoadCommand::LoadWeakDyLib(try!(Self::read_dylib::<O, T>(buf))),
            LC_REEXPORT_DYLIB => LoadCommand::ReexportDyLib(try!(Self::read_dylib::<O, T>(buf))),
            LC_LOAD_UPWARD_DYLIB => LoadCommand::LoadUpwardDylib(try!(Self::read_dylib::<O, T>(buf))),
            LC_LAZY_LOAD_DYLIB => LoadCommand::LazyLoadDylib(try!(Self::read_dylib::<O, T>(buf))),

            LC_ID_DYLINKER => LoadCommand::IdDyLinker(try!(Self::read_dylinker::<O, T>(buf))),
            LC_LOAD_DYLINKER => LoadCommand::LoadDyLinker(try!(Self::read_dylinker::<O, T>(buf))),
            LC_DYLD_ENVIRONMENT => LoadCommand::DyLdEnv(try!(Self::read_dylinker::<O, T>(buf))),

            LC_SYMTAB => {
                LoadCommand::SymTab {
                    symoff: try!(buf.read_u32::<O>()),
                    nsyms: try!(buf.read_u32::<O>()),
                    stroff: try!(buf.read_u32::<O>()),
                    strsize: try!(buf.read_u32::<O>()),
                }
            }
            LC_DYSYMTAB => {
                LoadCommand::DySymTab {
                    ilocalsym: try!(buf.read_u32::<O>()),
                    nlocalsym: try!(buf.read_u32::<O>()),
                    iextdefsym: try!(buf.read_u32::<O>()),
                    nextdefsym: try!(buf.read_u32::<O>()),
                    iundefsym: try!(buf.read_u32::<O>()),
                    nundefsym: try!(buf.read_u32::<O>()),
                    tocoff: try!(buf.read_u32::<O>()),
                    ntoc: try!(buf.read_u32::<O>()),
                    modtaboff: try!(buf.read_u32::<O>()),
                    nmodtab: try!(buf.read_u32::<O>()),
                    extrefsymoff: try!(buf.read_u32::<O>()),
                    nextrefsyms: try!(buf.read_u32::<O>()),
                    indirectsymoff: try!(buf.read_u32::<O>()),
                    nindirectsyms: try!(buf.read_u32::<O>()),
                    extreloff: try!(buf.read_u32::<O>()),
                    nextrel: try!(buf.read_u32::<O>()),
                    locreloff: try!(buf.read_u32::<O>()),
                    nlocrel: try!(buf.read_u32::<O>()),
                }
            }
            LC_UUID => {
                let mut uuid = [0; 16];

                try!(buf.read_exact(&mut uuid[..]));

                LoadCommand::Uuid(try!(Uuid::from_bytes(&uuid[..])))
            }
            LC_CODE_SIGNATURE => LoadCommand::CodeSignature(try!(Self::read_linkedit_data::<O, T>(buf))),
            LC_SEGMENT_SPLIT_INFO => LoadCommand::SegmentSplitInfo(try!(Self::read_linkedit_data::<O, T>(buf))),
            LC_FUNCTION_STARTS => LoadCommand::FunctionStarts(try!(Self::read_linkedit_data::<O, T>(buf))),
            LC_DATA_IN_CODE => LoadCommand::DataInCode(try!(Self::read_linkedit_data::<O, T>(buf))),
            LC_DYLIB_CODE_SIGN_DRS => LoadCommand::DylibCodeSignDrs(try!(Self::read_linkedit_data::<O, T>(buf))),
            LC_LINKER_OPTIMIZATION_HINT => {
                LoadCommand::LinkerOptimizationHint(try!(Self::read_linkedit_data::<O, T>(buf)))
            }

            LC_VERSION_MIN_MACOSX |
            LC_VERSION_MIN_IPHONEOS |
            LC_VERSION_MIN_WATCHOS |
            LC_VERSION_MIN_TVOS => {
                LoadCommand::VersionMin {
                    target: BuildTarget::from(cmd),
                    version: VersionTag(try!(buf.read_u32::<O>())),
                    sdk: VersionTag(try!(buf.read_u32::<O>())),
                }
            }
            LC_DYLD_INFO_ONLY => {
                LoadCommand::DyldInfo {
                    rebase_off: try!(buf.read_u32::<O>()),
                    rebase_size: try!(buf.read_u32::<O>()),
                    bind_off: try!(buf.read_u32::<O>()),
                    bind_size: try!(buf.read_u32::<O>()),
                    weak_bind_off: try!(buf.read_u32::<O>()),
                    weak_bind_size: try!(buf.read_u32::<O>()),
                    lazy_bind_off: try!(buf.read_u32::<O>()),
                    lazy_bind_size: try!(buf.read_u32::<O>()),
                    export_off: try!(buf.read_u32::<O>()),
                    export_size: try!(buf.read_u32::<O>()),
                }
            }
            LC_MAIN => {
                LoadCommand::EntryPoint {
                    entryoff: try!(buf.read_u64::<O>()),
                    stacksize: try!(buf.read_u64::<O>()),
                }
            }
            LC_SOURCE_VERSION => LoadCommand::SourceVersion(SourceVersionTag(try!(buf.read_u64::<O>()))),
            _ => {
                let mut payload = Vec::new();

                payload.resize(cmdsize as usize - 8, 0);

                debug!("load {} command with {} bytes payload",
                       LoadCommand::cmd_name(cmd),
                       payload.len());

                try!(buf.read_exact(payload.as_mut()));

                let cmd = LoadCommand::Command {
                    cmd: cmd,
                    payload: payload,
                };

                cmd
            }
        };

        debug!("parsed {} command: {:?}", cmd.name(), cmd);

        let read = (buf.position() - begin) as usize;

        // skip the reserved or padding bytes
        buf.consume(cmdsize as usize - read);

        Ok((cmd, cmdsize as usize))
    }

    fn read_lc_string<O: ByteOrder, T: AsRef<[u8]>>(buf: &mut Cursor<T>) -> Result<String> {
        let mut s = Vec::new();

        try!(buf.read_until(0, &mut s));

        let len = s.iter().position(|c| *c == 0x00).unwrap_or(s.len());
        Ok(String::from(try!(str::from_utf8(&s[..len]))))
    }

    fn read_dylinker<O: ByteOrder, T: AsRef<[u8]>>(buf: &mut Cursor<T>) -> Result<LcString> {
        let off = try!(buf.read_u32::<O>()) as usize;

        buf.consume(off - 12);

        Ok(LcString(off, try!(Self::read_lc_string::<O, T>(buf))))
    }

    fn read_fvmlib<O: ByteOrder, T: AsRef<[u8]>>(buf: &mut Cursor<T>) -> Result<FvmLib> {
        let off = try!(buf.read_u32::<O>()) as usize;
        let minor_version = try!(buf.read_u32::<O>());
        let header_addr = try!(buf.read_u32::<O>());

        buf.consume(off - 20);

        Ok(FvmLib {
            name: LcString(off, try!(Self::read_lc_string::<O, T>(buf))),
            minor_version: minor_version,
            header_addr: header_addr,
        })
    }

    fn read_dylib<O: ByteOrder, T: AsRef<[u8]>>(buf: &mut Cursor<T>) -> Result<DyLib> {
        let off = try!(buf.read_u32::<O>()) as usize;
        let timestamp = try!(buf.read_u32::<O>());
        let current_version = try!(buf.read_u32::<O>());
        let compatibility_version = try!(buf.read_u32::<O>());

        buf.consume(off - 24);

        Ok(DyLib {
            name: LcString(off, try!(Self::read_lc_string::<O, T>(buf))),
            timestamp: timestamp,
            current_version: VersionTag(current_version),
            compatibility_version: VersionTag(compatibility_version),
        })
    }

    fn read_linkedit_data<O: ByteOrder, T: AsRef<[u8]>>(buf: &mut Cursor<T>) -> Result<LinkEditData> {
        Ok(LinkEditData {
            off: try!(buf.read_u32::<O>()),
            size: try!(buf.read_u32::<O>()),
        })
    }

    pub fn cmd(&self) -> u32 {
        match self {
            &LoadCommand::Segment { .. } => LC_SEGMENT,
            &LoadCommand::Segment64 { .. } => LC_SEGMENT_64,
            &LoadCommand::IdFvmLib(_) => LC_IDFVMLIB,
            &LoadCommand::LoadFvmLib(_) => LC_LOADFVMLIB,
            &LoadCommand::IdDyLib(_) => LC_ID_DYLIB,
            &LoadCommand::LoadDyLib(_) => LC_LOAD_DYLIB,
            &LoadCommand::LoadWeakDyLib(_) => LC_LOAD_WEAK_DYLIB,
            &LoadCommand::ReexportDyLib(_) => LC_REEXPORT_DYLIB,
            &LoadCommand::LoadUpwardDylib(_) => LC_LOAD_UPWARD_DYLIB,
            &LoadCommand::LazyLoadDylib(_) => LC_LAZY_LOAD_DYLIB,
            &LoadCommand::IdDyLinker(_) => LC_ID_DYLINKER,
            &LoadCommand::LoadDyLinker(_) => LC_LOAD_DYLINKER,
            &LoadCommand::DyLdEnv(_) => LC_DYLD_ENVIRONMENT,
            &LoadCommand::SymTab { .. } => LC_SYMTAB,
            &LoadCommand::DySymTab { .. } => LC_DYSYMTAB,
            &LoadCommand::Uuid(_) => LC_UUID,
            &LoadCommand::CodeSignature(_) => LC_CODE_SIGNATURE,
            &LoadCommand::SegmentSplitInfo(_) => LC_SEGMENT_SPLIT_INFO,
            &LoadCommand::FunctionStarts(_) => LC_FUNCTION_STARTS,
            &LoadCommand::DataInCode(_) => LC_DATA_IN_CODE,
            &LoadCommand::DylibCodeSignDrs(_) => LC_DYLIB_CODE_SIGN_DRS,
            &LoadCommand::LinkerOptimizationHint(_) => LC_LINKER_OPTIMIZATION_HINT,
            &LoadCommand::VersionMin { target, .. } => BuildTarget::into(target),
            &LoadCommand::DyldInfo { .. } => LC_DYLD_INFO_ONLY,
            &LoadCommand::EntryPoint { .. } => LC_MAIN,
            &LoadCommand::SourceVersion(_) => LC_SOURCE_VERSION,
            &LoadCommand::Command { cmd, .. } => cmd,
        }
    }

    pub fn name(&self) -> &'static str {
        Self::cmd_name(self.cmd())
    }

    fn cmd_name(cmd: u32) -> &'static str {
        match cmd {
            LC_SEGMENT => "LC_SEGMENT",
            LC_SYMTAB => "LC_SYMTAB",
            LC_SYMSEG => "LC_SYMSEG",
            LC_THREAD => "LC_THREAD",
            LC_UNIXTHREAD => "LC_UNIXTHREAD",
            LC_LOADFVMLIB => "LC_LOADFVMLIB",
            LC_IDFVMLIB => "LC_IDFVMLIB",
            LC_IDENT => "LC_IDENT",
            LC_FVMFILE => "LC_FVMFILE",
            LC_PREPAGE => "LC_PREPAGE",
            LC_DYSYMTAB => "LC_DYSYMTAB",
            LC_LOAD_DYLIB => "LC_LOAD_DYLIB",
            LC_ID_DYLIB => "LC_ID_DYLIB",
            LC_LOAD_DYLINKER => "LC_LOAD_DYLINKER",
            LC_ID_DYLINKER => "LC_ID_DYLINKER",
            LC_PREBOUND_DYLIB => "LC_PREBOUND_DYLIB",
            LC_ROUTINES => "LC_ROUTINES",
            LC_SUB_FRAMEWORK => "LC_SUB_FRAMEWORK",
            LC_SUB_UMBRELLA => "LC_SUB_UMBRELLA",
            LC_SUB_CLIENT => "LC_SUB_CLIENT",
            LC_SUB_LIBRARY => "LC_SUB_LIBRARY",
            LC_TWOLEVEL_HINTS => "LC_TWOLEVEL_HINTS",
            LC_PREBIND_CKSUM => "LC_PREBIND_CKSUM",
            LC_LOAD_WEAK_DYLIB => "LC_LOAD_WEAK_DYLIB",
            LC_SEGMENT_64 => "LC_SEGMENT_64",
            LC_ROUTINES_64 => "LC_ROUTINES_64",
            LC_UUID => "LC_UUID",
            LC_RPATH => "LC_RPATH",
            LC_CODE_SIGNATURE => "LC_CODE_SIGNATURE",
            LC_SEGMENT_SPLIT_INFO => "LC_SEGMENT_SPLIT_INFO",
            LC_REEXPORT_DYLIB => "LC_REEXPORT_DYLIB",
            LC_LAZY_LOAD_DYLIB => "LC_LAZY_LOAD_DYLIB",
            LC_ENCRYPTION_INFO => "LC_ENCRYPTION_INFO",
            LC_DYLD_INFO => "LC_DYLD_INFO",
            LC_DYLD_INFO_ONLY => "LC_DYLD_INFO_ONLY",
            LC_LOAD_UPWARD_DYLIB => "LC_LOAD_UPWARD_DYLIB",
            LC_VERSION_MIN_MACOSX => "LC_VERSION_MIN_MACOSX",
            LC_VERSION_MIN_IPHONEOS => "LC_VERSION_MIN_IPHONEOS",
            LC_FUNCTION_STARTS => "LC_FUNCTION_STARTS",
            LC_DYLD_ENVIRONMENT => "LC_DYLD_ENVIRONMENT",
            LC_MAIN => "LC_MAIN",
            LC_DATA_IN_CODE => "LC_DATA_IN_CODE",
            LC_SOURCE_VERSION => "LC_SOURCE_VERSION",
            LC_DYLIB_CODE_SIGN_DRS => "LC_DYLIB_CODE_SIGN_DRS",
            LC_ENCRYPTION_INFO_64 => "LC_ENCRYPTION_INFO_64",
            LC_LINKER_OPTION => "LC_LINKER_OPTION",
            LC_LINKER_OPTIMIZATION_HINT => "LC_LINKER_OPTIMIZATION_HINT",
            _ => "LC_COMMAND",
        }
    }
}

/// The flags field of a section structure is separated into two parts a section
/// type and section attributes.
///
/// The section types are mutually exclusive (it can only have one type)
/// but the section attributes are not (it may have more than one attribute).
///
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct SectionFlags(u32);

impl SectionFlags {
    pub fn sect_type(self) -> u32 {
        self.0 & SECTION_TYPE
    }

    pub fn sect_attrs(self) -> SectionAttributes {
        SectionAttributes::from_bits_truncate(self.0 & SECTION_ATTRIBUTES)
    }
}

impl Into<u32> for SectionFlags {
    fn into(self) -> u32 {
        self.0
    }
}

/// A segment is made up of zero or more sections.
///
/// Non-MH_OBJECT files have all of their segments with the proper sections in each,
/// and padded to the specified segment alignment when produced by the link editor.
/// The first segment of a MH_EXECUTE and MH_FVMLIB format file contains the mach_header
/// and load commands of the object file before its first section.  The zero
/// fill sections are always last in their segment (in all formats).  This
/// allows the zeroed segment padding to be mapped into memory where zero fill
/// sections might be. The gigabyte zero fill sections, those with the section
/// type S_GB_ZEROFILL, can only be in a segment with sections of this type.
/// These segments are then placed after all other segments.
///
/// The MH_OBJECT format has all of its sections in one segment for
/// compactness.  There is no padding to a specified segment boundary and the
/// mach_header and load commands are not part of the segment.
///
/// Sections with the same section name, sectname, going into the same segment,
/// segname, are combined by the link editor.  The resulting section is aligned
/// to the maximum alignment of the combined sections and is the new section's
/// alignment.  The combined sections are aligned to their original alignment in
/// the combined section.  Any padded bytes to get the specified alignment are
/// zeroed.
///
/// The format of the relocation entries referenced by the reloff and nreloc
/// fields of the section structure for mach object files is described in the
/// header file <reloc.h>.
///
///
#[derive(Debug, Clone)]
pub struct Section {
    /// name of this section
    pub sectname: String,
    /// segment this section goes in
    pub segname: String,
    /// memory address of this section
    pub addr: usize,
    /// size in bytes of this section
    pub size: usize,
    /// file offset of this section
    pub offset: u32,
    /// section alignment (power of 2)
    pub align: u32,
    /// file offset of relocation entries
    pub reloff: u32,
    /// number of relocation entries
    pub nreloc: u32,
    // flags (section type and attributes)
    pub flags: SectionFlags,
    /// reserved (for offset or index)
    pub reserved1: u32,
    /// reserved (for count or sizeof)
    pub reserved2: u32,
    /// reserved
    pub reserved3: u32,
}

impl Section {
    fn parse_section<T: BufRead, O: ByteOrder>(buf: &mut T) -> Result<Section> {
        let section = Section {
            sectname: try!(buf.read_fixed_size_string(16)),
            segname: try!(buf.read_fixed_size_string(16)),
            addr: try!(buf.read_u32::<O>()) as usize,
            size: try!(buf.read_u32::<O>()) as usize,
            offset: try!(buf.read_u32::<O>()),
            align: try!(buf.read_u32::<O>()),
            reloff: try!(buf.read_u32::<O>()),
            nreloc: try!(buf.read_u32::<O>()),
            flags: SectionFlags(try!(buf.read_u32::<O>())),
            reserved1: try!(buf.read_u32::<O>()),
            reserved2: try!(buf.read_u32::<O>()),
            reserved3: 0,
        };

        Ok(section)
    }

    fn parse_section64<T: BufRead, O: ByteOrder>(buf: &mut T) -> Result<Section> {
        let section = Section {
            sectname: try!(buf.read_fixed_size_string(16)),
            segname: try!(buf.read_fixed_size_string(16)),
            addr: try!(buf.read_u64::<O>()) as usize,
            size: try!(buf.read_u64::<O>()) as usize,
            offset: try!(buf.read_u32::<O>()),
            align: try!(buf.read_u32::<O>()),
            reloff: try!(buf.read_u32::<O>()),
            nreloc: try!(buf.read_u32::<O>()),
            flags: SectionFlags(try!(buf.read_u32::<O>())),
            reserved1: try!(buf.read_u32::<O>()),
            reserved2: try!(buf.read_u32::<O>()),
            reserved3: try!(buf.read_u32::<O>()),
        };

        Ok(section)
    }
}

/// The LC_DATA_IN_CODE load commands uses a linkedit_data_command
/// to point to an array of data_in_code_entry entries.
///
/// Each entry describes a range of data in a code section.
///
pub struct DataInCodeEntry {
    pub offset: u32, // from mach_header to start of data range
    pub length: u16, // number of bytes in data range
    pub kind: u16, // a DICE_KIND_* value
}

#[cfg(test)]
pub mod tests {
    extern crate env_logger;

    use nostd_io::Cursor;
    use alloc::Vec;
    use alloc::String;
    use alloc::string::ToString;

    use byteorder::LittleEndian;

    use super::super::*;

    include!("testdata.rs");

    macro_rules! parse_command {
        ($buf:expr) => ({
            let mut buf = Vec::new();

            buf.extend_from_slice(&$buf[..]);

            let mut cur = Cursor::new(buf);

            LoadCommand::parse::<LittleEndian, Vec<u8>>(&mut cur).unwrap()
        })
    }

    #[test]
    fn test_parse_segments() {
        if let (LoadCommand::Segment64 { ref segname,
                                         vmaddr,
                                         vmsize,
                                         fileoff,
                                         filesize,
                                         maxprot,
                                         initprot,
                                         flags,
                                         ref sections },
                cmdsize) = parse_command!(LC_SEGMENT_64_PAGEZERO_DATA) {
            assert_eq!(cmdsize, 72);
            assert_eq!(segname, SEG_PAGEZERO);
            assert_eq!(vmaddr, 0);
            assert_eq!(vmsize, 0x0000000100000000);
            assert_eq!(fileoff, 0);
            assert_eq!(filesize, 0);
            assert_eq!(maxprot, 0);
            assert_eq!(initprot, 0);
            assert!(flags.is_empty());
            assert!(sections.is_empty());
        } else {
            panic!();
        }

        if let (LoadCommand::Segment64 { ref segname,
                                         vmaddr,
                                         vmsize,
                                         fileoff,
                                         filesize,
                                         maxprot,
                                         initprot,
                                         flags,
                                         ref sections },
                cmdsize) = parse_command!(LC_SEGMENT_64_TEXT_DATA) {
            assert_eq!(cmdsize, 712);
            assert_eq!(segname, SEG_TEXT);
            assert_eq!(vmaddr, 0x0000000100000000);
            assert_eq!(vmsize, 0x00000000001e3000);
            assert_eq!(fileoff, 0);
            assert_eq!(filesize, 0x1e3000);
            assert_eq!(maxprot, 7);
            assert_eq!(initprot, 5);
            assert!(flags.is_empty());
            assert_eq!(sections.len(), 8);

            assert_eq!(sections.iter()
                           .map(|ref sec| (*sec).sectname.clone())
                           .collect::<Vec<String>>(),
                       vec![SECT_TEXT,
                            "__stubs",
                            "__stub_helper",
                            "__gcc_except_tab",
                            "__const",
                            "__cstring",
                            "__unwind_info",
                            "__eh_frame"]);
        } else {
            panic!();
        }

        if let (LoadCommand::Segment64 { ref segname,
                                         vmaddr,
                                         vmsize,
                                         fileoff,
                                         filesize,
                                         maxprot,
                                         initprot,
                                         flags,
                                         ref sections },
                cmdsize) = parse_command!(LC_SEGMENT_64_DATA_DATA) {
            assert_eq!(cmdsize, 872);
            assert_eq!(segname, SEG_DATA);
            assert_eq!(vmaddr, 0x00000001001e3000);
            assert_eq!(vmsize, 0x0000000000013000);
            assert_eq!(fileoff, 0x1e3000);
            assert_eq!(filesize, 0x12000);
            assert_eq!(maxprot, 7);
            assert_eq!(initprot, 3);
            assert!(flags.is_empty());
            assert_eq!(sections.len(), 10);

            assert_eq!(sections.iter()
                           .map(|ref sec| (*sec).sectname.clone())
                           .collect::<Vec<String>>(),
                       vec!["__nl_symbol_ptr",
                            "__got",
                            "__la_symbol_ptr",
                            "__mod_init_func",
                            "__const",
                            SECT_DATA,
                            "__thread_vars",
                            "__thread_data",
                            SECT_COMMON,
                            SECT_BSS]);
        } else {
            panic!();
        }

        if let (LoadCommand::Segment64 { ref segname,
                                         vmaddr,
                                         vmsize,
                                         fileoff,
                                         filesize,
                                         maxprot,
                                         initprot,
                                         flags,
                                         ref sections },
                cmdsize) = parse_command!(LC_SEGMENT_64_LINKEDIT_DATA) {
            assert_eq!(cmdsize, 72);
            assert_eq!(segname, SEG_LINKEDIT);
            assert_eq!(vmaddr, 0x00000001001f6000);
            assert_eq!(vmsize, 0x000000000017a000);
            assert_eq!(fileoff, 0x1f5000);
            assert_eq!(filesize, 0x1790b4);
            assert_eq!(maxprot, 7);
            assert_eq!(initprot, 1);
            assert!(flags.is_empty());
            assert!(sections.is_empty());
        } else {
            panic!();
        }
    }

    #[test]
    fn test_parse_dyld_info_command() {
        if let (LoadCommand::DyldInfo { rebase_off,
                                        rebase_size,
                                        bind_off,
                                        bind_size,
                                        weak_bind_off,
                                        weak_bind_size,
                                        lazy_bind_off,
                                        lazy_bind_size,
                                        export_off,
                                        export_size },
                cmdsize) = parse_command!(LC_DYLD_INFO_ONLY_DATA) {
            assert_eq!(cmdsize, 48);
            assert_eq!(rebase_off, 0x1f5000);
            assert_eq!(rebase_size, 3368);
            assert_eq!(bind_off, 0x1f5d28);
            assert_eq!(bind_size, 80);
            assert_eq!(weak_bind_off, 0x1f5d78);
            assert_eq!(weak_bind_size, 24);
            assert_eq!(lazy_bind_off, 0x1f5d90);
            assert_eq!(lazy_bind_size, 1688);
            assert_eq!(export_off, 0x1f6428);
            assert_eq!(export_size, 34856);
        } else {
            panic!();
        }
    }

    #[test]
    fn test_parse_symtab_command() {
        if let (LoadCommand::SymTab { symoff, nsyms, stroff, strsize }, cmdsize) = parse_command!(LC_SYMTAB_DATA) {
            assert_eq!(cmdsize, 24);
            assert_eq!(symoff, 0x200d88);
            assert_eq!(nsyms, 36797);
            assert_eq!(stroff, 0x290bf4);
            assert_eq!(strsize, 906432);
        } else {
            panic!();
        }
    }

    #[test]
    fn test_parse_dysymtab_command() {
        if let (LoadCommand::DySymTab { ilocalsym,
                                        nlocalsym,
                                        iextdefsym,
                                        nextdefsym,
                                        iundefsym,
                                        nundefsym,
                                        tocoff,
                                        ntoc,
                                        modtaboff,
                                        nmodtab,
                                        extrefsymoff,
                                        nextrefsyms,
                                        indirectsymoff,
                                        nindirectsyms,
                                        extreloff,
                                        nextrel,
                                        locreloff,
                                        nlocrel },
                cmdsize) = parse_command!(LC_DYSYMTAB_DATA) {
            assert_eq!(cmdsize, 80);
            assert_eq!(ilocalsym, 0);
            assert_eq!(nlocalsym, 35968);
            assert_eq!(iextdefsym, 35968);
            assert_eq!(nextdefsym, 746);
            assert_eq!(iundefsym, 36714);
            assert_eq!(nundefsym, 83);
            assert_eq!(tocoff, 0);
            assert_eq!(ntoc, 0);
            assert_eq!(modtaboff, 0);
            assert_eq!(nmodtab, 0);
            assert_eq!(extrefsymoff, 0);
            assert_eq!(nextrefsyms, 0);
            assert_eq!(indirectsymoff, 2689368);
            assert_eq!(nindirectsyms, 167);
            assert_eq!(extreloff, 0);
            assert_eq!(nextrel, 0);
            assert_eq!(locreloff, 0);
            assert_eq!(nlocrel, 0);
        } else {
            panic!();
        }
    }

    #[test]
    fn test_parse_load_dylinker_command() {
        if let (LoadCommand::LoadDyLinker(LcString(off, ref name)), cmdsize) = parse_command!(LC_LOAD_DYLINKER_DATA) {
            assert_eq!(cmdsize, 32);
            assert_eq!(off, 12);
            assert_eq!(name, "/usr/lib/dyld");
        } else {
            panic!();
        }
    }

    #[test]
    fn test_parse_uuid_command() {
        if let (LoadCommand::Uuid(ref uuid), cmdsize) = parse_command!(LC_UUID_DATA) {
            assert_eq!(cmdsize, 24);
            assert_eq!(uuid.hyphenated().to_string(),
                       "92e3cf1f-20da-3373-a98c-851366d353bf");
        } else {
            panic!();
        }
    }

    #[test]
    fn test_parse_min_version_command() {
        if let (LoadCommand::VersionMin { target, version, sdk }, cmdsize) =
               parse_command!(LC_VERSION_MIN_MACOSX_DATA) {
            assert_eq!(cmdsize, 16);
            assert_eq!(target, BuildTarget::MacOsX);
            assert_eq!(version.to_string(), "10.11");
            assert_eq!(sdk.to_string(), "10.11");
        } else {
            panic!();
        }
    }

    #[test]
    fn test_parse_source_version_command() {
        if let (LoadCommand::SourceVersion(version), cmdsize) = parse_command!(LC_SOURCE_VERSION_DATA) {
            assert_eq!(cmdsize, 16);
            assert_eq!(version.to_string(), "0.0");
        } else {
            panic!();
        }
    }

    #[test]
    fn test_parse_main_command() {
        if let (LoadCommand::EntryPoint { entryoff, stacksize }, cmdsize) = parse_command!(LC_MAIN_DATA) {
            assert_eq!(cmdsize, 24);
            assert_eq!(entryoff, 0x11400);
            assert_eq!(stacksize, 0);
        } else {
            panic!();
        }
    }

    #[test]
    fn test_load_dylib_command() {
        if let (LoadCommand::LoadDyLib(ref dylib), cmdsize) = parse_command!(LC_LOAD_DYLIB_DATA) {
            assert_eq!(cmdsize, 56);
            assert_eq!(dylib.name,
                       LcString(24, String::from("/usr/lib/libSystem.B.dylib")));
            assert_eq!(dylib.timestamp, 2);
            assert_eq!(dylib.current_version.to_string(), "1226.10.1");
            assert_eq!(dylib.compatibility_version.to_string(), "1.0");
        } else {
            panic!();
        }
    }

    #[test]
    fn test_parse_link_edit_data_command() {
        if let (LoadCommand::FunctionStarts(LinkEditData { off, size }), cmdsize) =
               parse_command!(LC_FUNCTION_STARTS_DATA) {
            assert_eq!(cmdsize, 16);
            assert_eq!(off, 0x1fec50);
            assert_eq!(size, 8504);
        } else {
            panic!();
        }

        if let (LoadCommand::DataInCode(LinkEditData { off, size }), cmdsize) = parse_command!(LC_DATA_IN_CODE_DATA) {
            assert_eq!(cmdsize, 16);
            assert_eq!(off, 0x200d88);
            assert_eq!(size, 0);
        } else {
            panic!();
        }
    }
}
