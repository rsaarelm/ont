# Outline Note Tool

Utility tools and library for notes in [IDM](https://github.com/rsaarelm/idm/) format.

Ont consists of two basic building blocks on top of IDM.
It's intended for working with outline notes written in IDM, which have type `Outline` defined as a mutually recursive type:

```
type Outline = ((indexmap::IndexMap<String, String>,), Vec<Section>);
type Section = ((String,), Outline);
```

Ont has two fundamental parts: A mutating tree iterator for processing outline structure in memory and an uniform command-line interface for reading or writing outlines from files, directories or piped text streams.

## The mutable iterator

The type `ont::Outline` has methods `iter` and `iter_mut` that produce depth-first tree iterators that reference `Section` children of the outline and all the child outlines in the sections.
Methods `context_iter` and `context_iter_mut` construct iterators that yield pairs `(&mut C, & Section)` or `(&mut C, &mut Section)` where `C` is an user-defined type that is preserved when going down the outline tree branches and lets you carry information, like tags defined in a parent section to be inherited by children, down a branch.

Various outline-processing tools in ont are written as procedures that runs such an iterator over a collection of notes, possibly collecting information or modifying the iterated sections.

## The command-line interface

Ont operations are shell program invocations with an input and an output.
If nothing is specified, the default behavior is to read from the standard input and write to the standard output.
An input or an output can be specified as a valid file path, a valid directory path, or `-` for the standard input or output.
Files are read and written as you would expect.

Directories ("collections") are converted into a single outline.
The outline is constructed in a way that lets it be output back into a collection of files if the output is also a directory path.
Subdirectories become outline headlines ending with a slash, eg. `subdir/`.
The contents of the subdirectory are recursively read and inserted under the subdirectory's section.
Only files with extensions are read.
If a file has `.idm` extension, it is read as a headline without the extension, `structure.idm` becomes `structure`.
Files with any other extension become headlines with the same extension, `notes.txt` stays as `notes.txt`.
The contents of the file must be a valid IDM outline and are inserted as the outline child of the file's section.

When writing to a directory, the above logic is used in reverse.
Top-level headlines ending in slash generate subdirectories, other headlines generate files and everything under the headline becomes file contents.
Ont keeps track of which files were initially read, and if a mutable iteration has deleted outline sections corresponding to any of them and a directory is being modified in-place, it will delete the corresponding files from the collection directory.

Since the collection structure is fully inferred from the outline, you should be able to output a collection to a single outline file and then later output the single file to a new collection and retain the file and subdirectory structure of the original collection.
It's also straightforward to pipe multiple operations together with the intermediate steps being passed in standard input streams and the output becoming a multi-file collection again.

**Writing to a directory is dangerous, ont may delete files to make the directory contents match the collection being written. Bugs in ont or weird use cases can recursively delete your data. Use with caution and take backups.**

## The actual tools

Currently tools are written inside the ont binary.
These are pretty haphazard and less principled than the core ont functionality.

Some of the current ones:

* `cat`: Echo a collection back. Useful for testing the various output modes
  and for seeing whether a file or a collection is a valid IDM outline.

* `find-dupes`: Find duplicate link bookmarks or wiki definitions.

* `tagged`: List all entries that have the specific tags.
  Tags from a parent section are inherited by children.

* `tf`: Format a block of tabular IDM into nicely lined-up columns, try to
  align all-numeric columns to the right instead of to the left.

* `import-raindrop` and `export-raindrop`: Convert CSV export from [raindrop.io](https://raindrop.io/) bookmark manager to IDM notes
  and convert an IDM bookmark list to a Raindrop import CSV.

* `weave`: Execute script file fragments embedded in the outline and embed their output underneath the script.
  Have your very own interactive notebook system without a weird web server or an unreadable JSON-based internal save format.

  Weave syntax in the outline note:

  `>-`  *Embedded file indicator, use >filename.something if you want to output to a specific file*  
  `  #!/usr/bin/env python3`  *files starting with shebang are executed*  
  `  print("Hello, world!")`  
  `==`  *Output indicator, `weave` will overwrite its contents*  
  `  Hello, world!`  *Weaved output*
