// arboard Reference
// Cargo.toml: arboard = "3.4"
// Usage: use arboard::{*};

use arboard::{Clear, Clipboard, Get, ImageData, Set, Error};
use arboard::{LinuxClipboardKind, ClearExtLinux, GetExtLinux, SetExtLinux, Clear, Clipboard};
use arboard::{Get, ImageData, Set, Error, LinuxClipboardKind, ClearExtLinux};
use arboard::{GetExtLinux, SetExtLinux, Clear, Clipboard, Get, ImageData};
use arboard::{Set, Error, LinuxClipboardKind, ClearExtLinux, GetExtLinux, SetExtLinux};

// ============================================================================
// STRUCTS
// ============================================================================

pub struct Clear<'clipboard> { }         // A builder for an operation that clears the data from the clipboard.
clear.default(self) -> Result<()         // Completes the “clear” operation by deleting any existing clipboard data,
regardless of the format.
clear.clipboard(selection)
clear.type_id()
clear.borrow()
clear.borrow_mut()
clear.from(t)                            // Returns the argument unchanged.
clear.into()                             // Calls U::from(self).
clear.try_from(value)
clear.try_into()

pub struct Clipboard { }                 // The OS independent struct for accessing the clipboard.
clipboard.new()                          // Creates an instance of the clipboard.
clipboard.get_text()                     // Fetches UTF-8 text from the clipboard and returns it.
clipboard.set_text(text, ) -> Result<()  // Places the text onto the clipboard.
clipboard.set_html(html, alt_text, ) -> Result<() // Places the HTML as well as a plain-text alternative onto the clipboard.
clipboard.get_image()                    // Fetches image data from the clipboard, and returns the decoded pixels.
clipboard.set_image(image)               // Places an image to the clipboard.
clipboard.clear(&mut self) -> Result<()  // Clears any contents that may be present from the platform’s default clipboard,
regardless of the format of the data.
clipboard.clear_with()                   // Begins a “clear” option to remove data from the clipboard.
clipboard.get()                          // Begins a “get” operation to retrieve data from the clipboard.
clipboard.set()                          // Begins a “set” operation to set the clipboard’s contents.
clipboard.type_id()
clipboard.borrow()
clipboard.borrow_mut()
clipboard.from(t)                        // Returns the argument unchanged.
clipboard.into()                         // Calls U::from(self).
clipboard.try_from(value)
clipboard.try_into()

pub struct Get<'clipboard> { }           // A builder for an operation that gets a value from the clipboard.
get.text()                               // Completes the “get” operation by fetching UTF-8 text from the clipboard.
get.image()                              // Completes the “get” operation by fetching image data from the clipboard and returning the
decoded pixels.
get.html()                               // Completes the “get” operation by fetching HTML from the clipboard.
get.file_list()                          // Completes the “get” operation by fetching a list of file paths from the clipboard.
get.clipboard(selection)
get.type_id()
get.borrow()
get.borrow_mut()
get.from(t)                              // Returns the argument unchanged.
get.into()                               // Calls U::from(self).
get.try_from(value)
get.try_into()

pub struct ImageData<'a> { }             // Stores pixel data of an image.
image_data.into_owned_bytes()            // Returns a the bytes field in a way that it’s guaranteed to be owned.
It moves the bytes if they are already owned and...
image_data.to_owned_img()                // Returns an image data that is guaranteed to own its bytes.
It moves the bytes if they are already owned and clones them...
image_data.clone()
image_data.clone_from(source)
image_data.fmt(f)
image_data.type_id()
image_data.borrow()
image_data.borrow_mut()
image_data.clone_to_uninit(dest)
image_data.from(t)                       // Returns the argument unchanged.
image_data.into()                        // Calls U::from(self).
image_data.to_owned()
image_data.clone_into(target)
image_data.try_from(value)
image_data.try_into()

pub struct Set<'clipboard> { }           // A builder for an operation that sets a value to the clipboard.
set.text(text)                           // Completes the “set” operation by placing text onto the clipboard.
set.html(html, alt_text, ) -> Result<()  // Completes the “set” operation by placing HTML as well as a plain-text alternative onto the
clipboard.
set.image(image)                         // Completes the “set” operation by placing an image onto the clipboard.
set.file_list(file_list)                 // Completes the “set” operation by placing a list of file paths onto the clipboard.
set.wait()
set.clipboard(selection)
set.wait_until(deadline)
set.exclude_from_history()
set.type_id()
set.borrow()
set.borrow_mut()
set.from(t)                              // Returns the argument unchanged.
set.into()                               // Calls U::from(self).
set.try_from(value)
set.try_into()

#[non_exhaustive]pub enum Error { ContentNotAvailable, ClipboardNotSupported, ClipboardOccupied, ConversionFailure, Unknown { description: String, }, } // An error that might happen during a clipboard operation.
error.fmt(f)
error.source(&self) -> Option<&(dyn Error + 'static)
error.description()
error.cause()
error.provide(&'a self, request)
error.type_id()
error.borrow()
error.borrow_mut()
error.from(t)                            // Returns the argument unchanged.
error.into()                             // Calls U::from(self).
error.to_string()
error.try_from(value)
error.try_into()

pub enum LinuxClipboardKind { Clipboard, Primary, Secondary, } // Clipboard selection
linux_clipboard_kind.clone()
linux_clipboard_kind.clone_from(source)
linux_clipboard_kind.fmt(f)
linux_clipboard_kind.type_id()
linux_clipboard_kind.borrow()
linux_clipboard_kind.borrow_mut()
linux_clipboard_kind.clone_to_uninit(dest)
linux_clipboard_kind.from(t)             // Returns the argument unchanged.
linux_clipboard_kind.into()              // Calls U::from(self).
linux_clipboard_kind.to_owned()
linux_clipboard_kind.clone_into(target)
linux_clipboard_kind.try_from(value)
linux_clipboard_kind.try_into()

pub trait ClearExtLinux: Sealed { // Required method fn clipboard(self, selection: LinuxClipboardKind) -> Result<(), Error>; } // Linux specific extensions to the [Clear] builder.
clear_ext_linux.clipboard(selection)     // Performs the “clear” operation on the selected clipboard.

pub trait GetExtLinux: Sealed { // Required method fn clipboard(self, selection: LinuxClipboardKind) -> Self; } // Linux-specific extensions to the Get builder.
get_ext_linux.clipboard(selection)       // Sets the clipboard the operation will retrieve data from.

pub trait SetExtLinux: Sealed { // Required methods fn wait(self) -> Self; fn wait_until(self, deadline: Instant) -> Self; fn clipboard(self, selection: LinuxClipboardKind) -> Self; fn exclude_from_history(self) -> Self; } // Linux specific extensions to the Set builder.
set_ext_linux.wait()                     // Whether to wait for the clipboard’s contents to be replaced after setting it.
set_ext_linux.wait_until(deadline)       // Whether or not to wait for the clipboard’s content to be replaced after setting it.
set_ext_linux.clipboard(selection)       // Sets the clipboard the operation will store its data to.
set_ext_linux.exclude_from_history()     // Excludes the data which will be set on the clipboard from being added to
the desktop clipboard managers’ histories by...


// ============================================================================
// ENUMS
// ============================================================================

pub struct Clear<'clipboard> { /* private fields */ } // A builder for an operation that clears the data from the clipboard.
clear.default(self) -> Result<()         // Completes the “clear” operation by deleting any existing clipboard data,
regardless of the format.
clear.clipboard(selection)
clear.type_id()
clear.borrow()
clear.borrow_mut()
clear.from(t)                            // Returns the argument unchanged.
clear.into()                             // Calls U::from(self).
clear.try_from(value)
clear.try_into()

pub struct Clipboard { /* private fields */ } // The OS independent struct for accessing the clipboard.
clipboard.new()                          // Creates an instance of the clipboard.
clipboard.get_text()                     // Fetches UTF-8 text from the clipboard and returns it.
clipboard.set_text(text, ) -> Result<()  // Places the text onto the clipboard.
clipboard.set_html(html, alt_text, ) -> Result<() // Places the HTML as well as a plain-text alternative onto the clipboard.
clipboard.get_image()                    // Fetches image data from the clipboard, and returns the decoded pixels.
clipboard.set_image(image)               // Places an image to the clipboard.
clipboard.clear(&mut self) -> Result<()  // Clears any contents that may be present from the platform’s default clipboard,
regardless of the format of the data.
clipboard.clear_with()                   // Begins a “clear” option to remove data from the clipboard.
clipboard.get()                          // Begins a “get” operation to retrieve data from the clipboard.
clipboard.set()                          // Begins a “set” operation to set the clipboard’s contents.
clipboard.type_id()
clipboard.borrow()
clipboard.borrow_mut()
clipboard.from(t)                        // Returns the argument unchanged.
clipboard.into()                         // Calls U::from(self).
clipboard.try_from(value)
clipboard.try_into()

pub struct Get<'clipboard> { /* private fields */ } // A builder for an operation that gets a value from the clipboard.
get.text()                               // Completes the “get” operation by fetching UTF-8 text from the clipboard.
get.image()                              // Completes the “get” operation by fetching image data from the clipboard and returning the
decoded pixels.
get.html()                               // Completes the “get” operation by fetching HTML from the clipboard.
get.file_list()                          // Completes the “get” operation by fetching a list of file paths from the clipboard.
get.clipboard(selection)
get.type_id()
get.borrow()
get.borrow_mut()
get.from(t)                              // Returns the argument unchanged.
get.into()                               // Calls U::from(self).
get.try_from(value)
get.try_into()

pub struct ImageData<'a> { pub width: usize, pub height: usize, pub bytes: Cow<'a, [u8]>, } // Stores pixel data of an image.
image_data.into_owned_bytes()            // Returns a the bytes field in a way that it’s guaranteed to be owned.
It moves the bytes if they are already owned and...
image_data.to_owned_img()                // Returns an image data that is guaranteed to own its bytes.
It moves the bytes if they are already owned and clones them...
image_data.clone()
image_data.clone_from(source)
image_data.fmt(f)
image_data.type_id()
image_data.borrow()
image_data.borrow_mut()
image_data.clone_to_uninit(dest)
image_data.from(t)                       // Returns the argument unchanged.
image_data.into()                        // Calls U::from(self).
image_data.to_owned()
image_data.clone_into(target)
image_data.try_from(value)
image_data.try_into()

pub struct Set<'clipboard> { /* private fields */ } // A builder for an operation that sets a value to the clipboard.
set.text(text)                           // Completes the “set” operation by placing text onto the clipboard.
set.html(html, alt_text, ) -> Result<()  // Completes the “set” operation by placing HTML as well as a plain-text alternative onto the
clipboard.
set.image(image)                         // Completes the “set” operation by placing an image onto the clipboard.
set.file_list(file_list)                 // Completes the “set” operation by placing a list of file paths onto the clipboard.
set.wait()
set.clipboard(selection)
set.wait_until(deadline)
set.exclude_from_history()
set.type_id()
set.borrow()
set.borrow_mut()
set.from(t)                              // Returns the argument unchanged.
set.into()                               // Calls U::from(self).
set.try_from(value)
set.try_into()

pub enum Error { }                       // An error that might happen during a clipboard operation.
error.fmt(f)
error.source(&self) -> Option<&(dyn Error + 'static)
error.description()
error.cause()
error.provide(&'a self, request)
error.type_id()
error.borrow()
error.borrow_mut()
error.from(t)                            // Returns the argument unchanged.
error.into()                             // Calls U::from(self).
error.to_string()
error.try_from(value)
error.try_into()

pub enum LinuxClipboardKind { }          // Clipboard selection
linux_clipboard_kind.clone()
linux_clipboard_kind.clone_from(source)
linux_clipboard_kind.fmt(f)
linux_clipboard_kind.type_id()
linux_clipboard_kind.borrow()
linux_clipboard_kind.borrow_mut()
linux_clipboard_kind.clone_to_uninit(dest)
linux_clipboard_kind.from(t)             // Returns the argument unchanged.
linux_clipboard_kind.into()              // Calls U::from(self).
linux_clipboard_kind.to_owned()
linux_clipboard_kind.clone_into(target)
linux_clipboard_kind.try_from(value)
linux_clipboard_kind.try_into()

pub trait ClearExtLinux: Sealed { // Required method fn clipboard(self, selection: LinuxClipboardKind) -> Result<(), Error>; } // Linux specific extensions to the [Clear] builder.
clear_ext_linux.clipboard(selection)     // Performs the “clear” operation on the selected clipboard.

pub trait GetExtLinux: Sealed { // Required method fn clipboard(self, selection: LinuxClipboardKind) -> Self; } // Linux-specific extensions to the Get builder.
get_ext_linux.clipboard(selection)       // Sets the clipboard the operation will retrieve data from.

pub trait SetExtLinux: Sealed { // Required methods fn wait(self) -> Self; fn wait_until(self, deadline: Instant) -> Self; fn clipboard(self, selection: LinuxClipboardKind) -> Self; fn exclude_from_history(self) -> Self; } // Linux specific extensions to the Set builder.
set_ext_linux.wait()                     // Whether to wait for the clipboard’s contents to be replaced after setting it.
set_ext_linux.wait_until(deadline)       // Whether or not to wait for the clipboard’s content to be replaced after setting it.
set_ext_linux.clipboard(selection)       // Sets the clipboard the operation will store its data to.
set_ext_linux.exclude_from_history()     // Excludes the data which will be set on the clipboard from being added to
the desktop clipboard managers’ histories by...


// ============================================================================
// TRAITS
// ============================================================================

pub struct Clear<'clipboard> { /* private fields */ } // A builder for an operation that clears the data from the clipboard.
clear.default(self) -> Result<()         // Completes the “clear” operation by deleting any existing clipboard data,
regardless of the format.
clear.clipboard(selection)
clear.type_id()
clear.borrow()
clear.borrow_mut()
clear.from(t)                            // Returns the argument unchanged.
clear.into()                             // Calls U::from(self).
clear.try_from(value)
clear.try_into()

pub struct Clipboard { /* private fields */ } // The OS independent struct for accessing the clipboard.
clipboard.new()                          // Creates an instance of the clipboard.
clipboard.get_text()                     // Fetches UTF-8 text from the clipboard and returns it.
clipboard.set_text(text, ) -> Result<()  // Places the text onto the clipboard.
clipboard.set_html(html, alt_text, ) -> Result<() // Places the HTML as well as a plain-text alternative onto the clipboard.
clipboard.get_image()                    // Fetches image data from the clipboard, and returns the decoded pixels.
clipboard.set_image(image)               // Places an image to the clipboard.
clipboard.clear(&mut self) -> Result<()  // Clears any contents that may be present from the platform’s default clipboard,
regardless of the format of the data.
clipboard.clear_with()                   // Begins a “clear” option to remove data from the clipboard.
clipboard.get()                          // Begins a “get” operation to retrieve data from the clipboard.
clipboard.set()                          // Begins a “set” operation to set the clipboard’s contents.
clipboard.type_id()
clipboard.borrow()
clipboard.borrow_mut()
clipboard.from(t)                        // Returns the argument unchanged.
clipboard.into()                         // Calls U::from(self).
clipboard.try_from(value)
clipboard.try_into()

pub struct Get<'clipboard> { /* private fields */ } // A builder for an operation that gets a value from the clipboard.
get.text()                               // Completes the “get” operation by fetching UTF-8 text from the clipboard.
get.image()                              // Completes the “get” operation by fetching image data from the clipboard and returning the
decoded pixels.
get.html()                               // Completes the “get” operation by fetching HTML from the clipboard.
get.file_list()                          // Completes the “get” operation by fetching a list of file paths from the clipboard.
get.clipboard(selection)
get.type_id()
get.borrow()
get.borrow_mut()
get.from(t)                              // Returns the argument unchanged.
get.into()                               // Calls U::from(self).
get.try_from(value)
get.try_into()

pub struct ImageData<'a> { pub width: usize, pub height: usize, pub bytes: Cow<'a, [u8]>, } // Stores pixel data of an image.
image_data.into_owned_bytes()            // Returns a the bytes field in a way that it’s guaranteed to be owned.
It moves the bytes if they are already owned and...
image_data.to_owned_img()                // Returns an image data that is guaranteed to own its bytes.
It moves the bytes if they are already owned and clones them...
image_data.clone()
image_data.clone_from(source)
image_data.fmt(f)
image_data.type_id()
image_data.borrow()
image_data.borrow_mut()
image_data.clone_to_uninit(dest)
image_data.from(t)                       // Returns the argument unchanged.
image_data.into()                        // Calls U::from(self).
image_data.to_owned()
image_data.clone_into(target)
image_data.try_from(value)
image_data.try_into()

pub struct Set<'clipboard> { /* private fields */ } // A builder for an operation that sets a value to the clipboard.
set.text(text)                           // Completes the “set” operation by placing text onto the clipboard.
set.html(html, alt_text, ) -> Result<()  // Completes the “set” operation by placing HTML as well as a plain-text alternative onto the
clipboard.
set.image(image)                         // Completes the “set” operation by placing an image onto the clipboard.
set.file_list(file_list)                 // Completes the “set” operation by placing a list of file paths onto the clipboard.
set.wait()
set.clipboard(selection)
set.wait_until(deadline)
set.exclude_from_history()
set.type_id()
set.borrow()
set.borrow_mut()
set.from(t)                              // Returns the argument unchanged.
set.into()                               // Calls U::from(self).
set.try_from(value)
set.try_into()

#[non_exhaustive]pub enum Error { ContentNotAvailable, ClipboardNotSupported, ClipboardOccupied, ConversionFailure, Unknown { description: String, }, } // An error that might happen during a clipboard operation.
error.fmt(f)
error.source(&self) -> Option<&(dyn Error + 'static)
error.description()
error.cause()
error.provide(&'a self, request)
error.type_id()
error.borrow()
error.borrow_mut()
error.from(t)                            // Returns the argument unchanged.
error.into()                             // Calls U::from(self).
error.to_string()
error.try_from(value)
error.try_into()

pub enum LinuxClipboardKind { Clipboard, Primary, Secondary, } // Clipboard selection
linux_clipboard_kind.clone()
linux_clipboard_kind.clone_from(source)
linux_clipboard_kind.fmt(f)
linux_clipboard_kind.type_id()
linux_clipboard_kind.borrow()
linux_clipboard_kind.borrow_mut()
linux_clipboard_kind.clone_to_uninit(dest)
linux_clipboard_kind.from(t)             // Returns the argument unchanged.
linux_clipboard_kind.into()              // Calls U::from(self).
linux_clipboard_kind.to_owned()
linux_clipboard_kind.clone_into(target)
linux_clipboard_kind.try_from(value)
linux_clipboard_kind.try_into()

pub trait ClearExtLinux: Sealed { }      // Linux specific extensions to the [Clear] builder.
clear_ext_linux.clipboard(selection)     // Performs the “clear” operation on the selected clipboard.

pub trait GetExtLinux: Sealed { }        // Linux-specific extensions to the Get builder.
get_ext_linux.clipboard(selection)       // Sets the clipboard the operation will retrieve data from.

pub trait SetExtLinux: Sealed { }        // Linux specific extensions to the Set builder.
set_ext_linux.wait()                     // Whether to wait for the clipboard’s contents to be replaced after setting it.
set_ext_linux.wait_until(deadline)       // Whether or not to wait for the clipboard’s content to be replaced after setting it.
set_ext_linux.clipboard(selection)       // Sets the clipboard the operation will store its data to.
set_ext_linux.exclude_from_history()     // Excludes the data which will be set on the clipboard from being added to
the desktop clipboard managers’ histories by...


