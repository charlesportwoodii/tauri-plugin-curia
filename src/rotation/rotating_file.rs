//! The one module here that touches the disk.
//!
//! Every decision it acts on comes from [`crate::rotation::policy`] and every
//! name it uses comes from [`crate::rotation::naming`], so what is left is the
//! sequencing: when to close a handle, when to rename, when to delete.

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use super::naming::{Archive, Stamp};
use super::policy::{ActiveFileFate, Policy, WriteAction};
use crate::{Error, FileOpenStrategy, RotationStrategy, TimezoneStrategy};

/// A log file that archives itself once it reaches a size limit.
///
/// Writes go to an in-memory buffer and reach the operating system on
/// [`Write::flush`], which is also where the rotation decision is taken. A
/// caller that writes one line and flushes it therefore never splits a line
/// across two files.
pub struct RotatingFile {
    dir: PathBuf,
    file_name: String,
    max_size: u64,
    rotation_strategy: RotationStrategy,
    timezone_strategy: TimezoneStrategy,
    /// `None` only for the moment between closing the old handle and opening
    /// the replacement, in the middle of a rotation.
    file: Option<File>,
    /// The size of the active file, tracked here rather than read back from the
    /// filesystem on every write.
    current_size: u64,
    buffer: Vec<u8>,
}

impl RotatingFile {
    /// Opens `<dir>/<file_name>.log`, creating it when it is not there.
    ///
    /// An existing file is either continued or archived, depending on
    /// `file_open_strategy` and on how much it already holds. Archives left
    /// over by a previous run under a larger budget are pruned here too: this
    /// is the only chance to notice that the configuration changed.
    pub fn new(
        dir: impl AsRef<Path>,
        file_name: String,
        max_size: u64,
        rotation_strategy: RotationStrategy,
        timezone_strategy: TimezoneStrategy,
        file_open_strategy: FileOpenStrategy,
    ) -> Result<Self, Error> {
        let dir = dir.as_ref().to_path_buf();
        let file = Self::open_for_append(&dir.join(Self::active_name(&file_name)))?;
        let current_size = file.metadata()?.len();

        let mut rotating = Self {
            dir,
            file_name,
            max_size,
            rotation_strategy,
            timezone_strategy,
            file: Some(file),
            current_size,
            buffer: Vec::new(),
        };

        if Policy::on_open(&file_open_strategy, current_size, max_size) == WriteAction::Rotate {
            rotating.rotate()?;
        }

        rotating.prune()?;

        Ok(rotating)
    }

    /// The name of the active log file, the only file that is written to.
    fn active_name(file_name: &str) -> String {
        let extension = Archive::EXTENSION;

        format!("{file_name}{extension}")
    }

    /// Where the active file lives.
    fn active_path(&self) -> PathBuf {
        self.dir.join(Self::active_name(&self.file_name))
    }

    /// Archives or discards the active file, deletes whatever the strategy no
    /// longer allows, and opens an empty active file in its place.
    fn rotate(&mut self) -> Result<(), Error> {
        let (archives, taken) = self.scan()?;
        let plan = Policy::resolve_rotation(&self.rotation_strategy, archives.len());
        let path = self.active_path();

        // Close the handle before the file moves. On Windows a rename can be
        // refused while a handle is open, and on Unix the handle would follow
        // the file into the archive and keep appending to it.
        self.file = None;

        match plan.active_file {
            ActiveFileFate::Archive => {
                let stamp = Stamp::render(self.timezone_strategy.get_now());
                let target = Archive::pick_free_name(&self.file_name, &stamp, &taken);

                // A rename onto a name nothing else holds, so no earlier
                // archive can be overwritten. An active file that vanished
                // underneath us leaves nothing to archive, which is not worth
                // failing the write that triggered the rotation.
                match std::fs::rename(&path, self.dir.join(target)) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(Error::Io(error)),
                }
            }
            ActiveFileFate::Discard => Self::remove_if_present(&path)?,
        }

        // Only archives that were already on disk are candidates. The one just
        // created is this rotation's own output.
        self.delete_oldest(&archives, plan.delete_oldest)?;

        self.file = Some(Self::open_fresh(&path)?);
        self.current_size = 0;

        Ok(())
    }

    /// Brings the directory back within the configured budget at startup.
    fn prune(&self) -> Result<(), Error> {
        let (archives, _) = self.scan()?;
        let doomed = Policy::prune_on_open(&self.rotation_strategy, archives.len());

        self.delete_oldest(&archives, doomed)
    }

    /// Deletes the first `count` of `archives`, which are ordered oldest first.
    fn delete_oldest(&self, archives: &[Archive], count: usize) -> Result<(), Error> {
        for archive in archives.iter().take(count) {
            Self::remove_if_present(&self.dir.join(&archive.name))?;
        }

        Ok(())
    }

    /// Reads the directory once, returning our archives oldest first and the
    /// names of everything in there.
    ///
    /// The second half is what a rotation checks a candidate archive name
    /// against, and it deliberately includes files that are not ours: the point
    /// is to avoid writing over anything at all.
    fn scan(&self) -> Result<(Vec<Archive>, HashSet<String>), Error> {
        let mut archives = Vec::new();
        let mut taken = HashSet::new();

        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;

            // Every name this crate writes is ASCII, so anything the platform
            // will not hand back as UTF-8 is not ours and not in our way.
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };

            let is_file = matches!(entry.file_type(), Ok(kind) if kind.is_file());
            taken.insert(name.clone());

            if !is_file {
                continue;
            }

            if let Some(archive) = Archive::parse(&self.file_name, &name) {
                archives.push(archive);
            }
        }

        archives.sort();

        Ok((archives, taken))
    }

    /// Opens the active file for appending, creating it when absent.
    fn open_for_append(path: &Path) -> Result<File, Error> {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(Error::Io)
    }

    /// Opens a replacement active file after a rotation. Truncating is belt and
    /// braces: the rotation has just moved or deleted whatever was there.
    fn open_fresh(path: &Path) -> Result<File, Error> {
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .map_err(Error::Io)
    }

    /// Deletes a file, treating an already absent one as success.
    fn remove_if_present(path: &Path) -> Result<(), Error> {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::Io(error)),
        }
    }
}

impl Write for RotatingFile {
    /// Buffers `buf`. Nothing reaches the operating system until [`Self::flush`].
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(buf);

        Ok(buf.len())
    }

    /// Rotates if the buffered bytes would not fit, then writes them.
    fn flush(&mut self) -> std::io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let buffered = self.buffer.len() as u64;

        if Policy::on_write(self.current_size, buffered, self.max_size) == WriteAction::Rotate {
            self.rotate().map_err(std::io::Error::other)?;
        }

        let Self {
            file,
            buffer,
            current_size,
            ..
        } = self;

        let outcome = match file.as_mut() {
            Some(file) => file.write_all(buffer),
            None => Err(std::io::Error::other("the active log file is not open")),
        };

        // Cleared either way. Bytes that failed to reach the file are gone, but
        // replaying them on the next flush would duplicate whatever part of
        // them did land.
        buffer.clear();
        outcome?;

        *current_size = current_size.saturating_add(buffered);

        Ok(())
    }
}

impl Drop for RotatingFile {
    /// Buffered bytes are written out on the way down, the same courtesy
    /// [`std::io::BufWriter`] extends. There is nobody left to report a failure
    /// to at this point.
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::RotatingFile;

    #[test]
    fn the_active_file_is_the_name_with_a_log_extension() {
        assert_eq!(RotatingFile::active_name("app"), "app.log");
    }
}
