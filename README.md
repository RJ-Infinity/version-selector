# Version Selector

this is a program that lets you select versions of different programs

(note it is currently only tested on windows so use at your own risk)

## quick start
```shell
# skip this if you are downloading release version this is only if you are building from source
$ cargo build --release
   Compiling version-selector v0.1.0
    Finished release [optimized] target(s) in 0.0s

# path to version-selector (usualy `./target/release/version-selector.exe` if building from source)
# if downloading release use the path to the downloaded executable
$ copy /path/to/version-selector.exe /path/to/app/select-appname-version.exe

$ cd /path/to/app/

# this command creates the default config file
$ ./select-appname-version.exe list
Could not read config file `version-selector.json` due to error `The system cannot find the file specified. (os error 2)`. so created a default config file.
Error reading file.

# replace ed with your favorite editor
$ ed ./version-selector.json
#replace the prefix with with the app file prefix and optionaly update the other values

#now you can select versions
$ ./select-appname-version.exe select
```

## commands
### help:
> this is the help command

takes one optional argument of the command to show information about
### list:
> lists all installed versions

takes no arguments.
### select:
> selects a version

takes an optional argument of the version name to select.

if no arguments given starts an interactive list to select the version from.

## how it works
the program creates a symlink to the file or dir that is selected
