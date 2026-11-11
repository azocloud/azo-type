== Core Native Desktop Architecture

By targeting native desktop platforms (*Linux*, *macOS*, and *Windows*) directly, our IDE works seamlessly with standard directories and files on disk:

+ *Direct Filesystem Project Tree (`DiskProject`):* Reads and writes `.typ`, `.bib`, and image assets directly in your chosen project folder on disk.
+ *Filesystem-Backed `EditorWorld`:* Resolves multi-file `#import` and `#include` statements directly from the project root directory on disk.
+ *Platform-Aware Menu Bar & Settings Window:* Uses the native OS screen-top menu bar on macOS and an integrated desktop menu bar on Linux/Windows, including `File > Settings` for live configuration.
