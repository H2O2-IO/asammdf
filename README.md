asammdf
==========

Read and write ASAM MDF file, simply and efficiently.

Features
--------

* Fast, use rust & mmap(todo) to boost parsing and writing speed.
* Support MDFv3(up to 3.3), and MDFv4(up to 4.1).
    * For MDF v4.1, support DZ block which compress data using deflate or transpose method.
* Easy to use with multi-thread program.

Examples
--------

Read a MDFv3 file, and then write data which recordindex=0 to console.

```rust
use asammdf::{MDFFile,SpecVer,v3,MDFObject,IDObject, ValueFormat};
let mut file = MDFFile::new();
file.open("./mdf3.dat").unwrap();
let idblock = file.get_id::<v3::IDBlock>().unwrap();
// MDF file's magic header is "MDF     "
assert_eq!(idblock.file_id(), "MDF     ");
assert_eq!(idblock.version(), 300);
// get all channel blocks out of MDF file
let iter = file.get_node_ids::<v3::CNBlock>().unwrap().into_iter();
// get all record data with recordindex=0 of these channel blocks
let _: Vec<f64> = iter.map(|node_id| {
        let data = file.get_data_cnblock(ValueFormat::Physical, node_id, 0);
        let name = file.get_node_by_id::<v3::CNBlock>(node_id).unwrap().name();
        println!("({name},{data})");
        data
    })
    .collect();
```
