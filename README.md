# Paged data structures

<!-- cargo-rdme start -->

This library provides a simple binary data format to organize large lists of data for random read-only access.

### Anatomy of a paged file

Data is organized in sections, pages and entries. Each file can include multiple sections, each section include multiple pages, and each page multiple entries. The first section/page may start at any given offset to give room to a potential file header. The file may also include one or more heap sections storing dynamically sized data.

A typical paged file will look like this:
```rust
┏━━━━━━━━━━━┓
┃  Header   ┃
┃           ┃
┗━━━━━━━━━━━┛
┏━━━━━━━━━━━┓
┃ Section 1 ┃
┃┌─────────┐┃
┃│ Page 1  │┃
┃├─────────┤┃
┃│ Entry 1 │┃
┃│   ...   │┃
┃│ Entry N │┃
┃└─────────┘┃
┃    ...    ┃
┃┌─────────┐┃
┃│ Page M  │┃
┃├─────────┤┃
┃│ Entry 1 │┃
┃│   ...   │┃
┃│ Entry N │┃
┃└─────────┘┃
┗━━━━━━━━━━━┛
     ...
┏━━━━━━━━━━━┓
┃ Section P ┃
┃┌─────────┐┃
┃│ Page 1  │┃
┃├─────────┤┃
┃│ Entry 1 │┃
┃│   ...   │┃
┃│ Entry N │┃
┃└─────────┘┃
┃    ...    ┃
┃┌─────────┐┃
┃│ Page M  │┃
┃├─────────┤┃
┃│ Entry 1 │┃
┃│   ...   │┃
┃│ Entry N │┃
┃└─────────┘┃
┗━━━━━━━━━━━┛
┏━━━━━━━━━━━┓
┃   Heap    ┃
┃           ┃
┃           ┃
┃           ┃
┃           ┃
┃           ┃
┃           ┃
┗━━━━━━━━━━━┛
```

#### Entry

The "entry" is the smallest unit of information in a file managed by this library. An entry represents any data whose type implements the `EncodeOnHeap` and `DecodeFromHeap` traits.

#### Page

A page is a list of entries of the same type. Every page of a file have the same byte length. All the entries in a page must have the same size. It is however possible for an entry to reference dynamically sized data living on a heap section.

#### Sections

A section is a list of pages of the same type. The size of a section is a multiple of the page length.

#### Heaps

A file may contain one or more heap sections. A heap stores dynamically sized data without any structure.

<!-- cargo-rdme end -->

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
