//! Archive file names: how one is rendered, and how one is recognised again.
//!
//! Nothing here touches the filesystem. The writer supplies the set of names
//! already present in the directory, and these types decide the rest.

mod archive;
mod stamp;

pub use archive::Archive;
pub use stamp::Stamp;
