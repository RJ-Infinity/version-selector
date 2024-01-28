#![feature(absolute_path)]

const VERSION: &str = "v1.1.0";

use std::{fs, env, path::{self, PathBuf}, io::{self, Write}};
use RJJSONrust::JSON;
use const_format::formatcp;

macro_rules! unwrap_or {($in: expr, $match: pat => $or: expr) =>{
    match $in{Err($match)=>$or,Ok(v)=>v}
};}
macro_rules! unwrap_or_exit {
    ($in: expr, $out: expr) => {unwrap_or!($in, _=>{return $out;})};
    ($in: expr) => {unwrap_or_exit!($in, ())};
}
macro_rules! unwrap_or_log_exit {
    ($in: expr, $out: expr) => {
        unwrap_or!($in, e=>{
            eprintln!("{}",e);
            return $out;
        })
    };
    ($in: expr) => {unwrap_or_log_exit!($in, ())};
}

mod symlink{
    /// note this entire module is untested on anything other than windows
    use std::{fs::{remove_dir_all, remove_file}, path::Path};
    #[cfg(target_os = "windows")]
    use std::os::windows::fs::{symlink_file, symlink_dir};
    use std::fs::{metadata, symlink_metadata};

    macro_rules! metadata_unwrap {($fn:ident ($path:ident)) => {
        unwrap_or!(($fn ($path)), e=>return Err(match e.kind(){
            std::io::ErrorKind::NotFound => ErrorKind::PathDoesNotExist($path),
            std::io::ErrorKind::PermissionDenied => ErrorKind::PermisionError,
            e=>panic!("{:#?}",e),
        }))
    };}

    #[derive(Debug)]
    pub enum ErrorKind<'a, P:AsRef<Path>>{
        PathDoesNotExist(&'a P),
        PathNotSymlink(&'a P),
        PathAlreadyExists(&'a P),
        PermisionError,
    }
    impl<P:AsRef<Path>> std::fmt::Display for ErrorKind<'_, P>{
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {match self{
            ErrorKind::PathDoesNotExist(path) => write!(f, "path `{}` does not exist", path.as_ref().display()),
            ErrorKind::PathNotSymlink(path) => write!(f, "path `{}` is not a symlink", path.as_ref().display()),
            ErrorKind::PathAlreadyExists(path) => write!(f, "path `{}` already exists", path.as_ref().display()),
            ErrorKind::PermisionError => write!(f, "permision error"),
        }}
    }
    #[cfg(target_os = "unix")]
    pub fn create_symlink<'a, P:AsRef<Path>>(orig: &'a P, link: &'a P)->Result<(),ErrorKind<'a, P>>
    {match std::os::unix::fs::symlink(orig, link){Ok(_)=>Ok(()),Err(e)=>match e.kind(){
        std::io::ErrorKind::PermissionDenied=>Err(PermisionError),
        e=>panic!("{:#?}",e),
    }}}
    #[cfg(target_os = "windows")]
    pub fn create_symlink<'a, P:AsRef<Path>>(orig: &'a P, link: &'a P)->Result<(),ErrorKind<'a, P>>{
        if let Ok(_) = metadata(link){return Err(ErrorKind::PathAlreadyExists(link));};
        let md = metadata_unwrap!(metadata(orig));
        return match (
            if md.is_dir(){symlink_dir}
            else if md.is_file(){symlink_file}
            else{panic!("path neither file or dir")}
        )(orig, link){
            Ok(())=>Ok(()),
            Err(e)=>return Err(match e.kind(){
                std::io::ErrorKind::NotFound => ErrorKind::PathDoesNotExist(orig),
                std::io::ErrorKind::PermissionDenied => {
                    eprintln!("{}",e);
                    eprintln!("{}",e.to_string());
                    ErrorKind::PermisionError
                },
                std::io::ErrorKind::AlreadyExists => ErrorKind::PathAlreadyExists(link),
                e=>panic!("{:#?}",e),
            })
        }
    }
    pub fn remove_symlink<'a, P:AsRef<Path>>(link: &'a P)->Result<(),ErrorKind<'a, P>>{
        let symlink_md = metadata_unwrap!(symlink_metadata(link));
        if !symlink_md.is_symlink(){return Err(ErrorKind::PathNotSymlink(link))}
        let md = metadata_unwrap!(metadata(link));
        if md.is_file(){return match remove_file(link){
            Ok(_)=>Ok(()),
            Err(e)=>Err(match e.kind(){
                std::io::ErrorKind::NotFound => ErrorKind::PathDoesNotExist(link),
                std::io::ErrorKind::PermissionDenied => ErrorKind::PermisionError,
                e=>panic!("{:#?}",e),
            }),
        }}else if md.is_dir(){return match remove_dir_all(link){
            Ok(_)=>Ok(()),
            Err(e)=>Err(match e.kind(){
                std::io::ErrorKind::NotFound => ErrorKind::PathDoesNotExist(link),
                std::io::ErrorKind::PermissionDenied => ErrorKind::PermisionError,
                e=>panic!("{:#?}",e),
            }),
        }}else{panic!("path neither file or dir");}
    }
}

const JSON_FILE_PATH: &str = "version-selector.json";
const PATH_PREFIX_KEY: &str = "pathPrefix";
const OUTPUT_PATH_KEY: &str = "outputPath";
const RELATIVE_PATH_KEY: &str = "relativePath";
const DEFAULT_JSON: &str = formatcp!(
    "{{\n\t\"{}\": \"app-\",\n\t\"{}\": \"app-selected\",\n\t\"{}\": \".\"\n}}",
    PATH_PREFIX_KEY,
    OUTPUT_PATH_KEY,
    RELATIVE_PATH_KEY,
);


struct Settings{
    path_prefix: String,
    output_path: PathBuf,
    dir_path: PathBuf,
}
impl Settings{
    fn new_from_path<P: AsRef<std::path::Path>>(path: P, abs_path: PathBuf)->Result<Self, String>{
        let json = unwrap_or!(fs::read_to_string(&path), err=>{
            eprint!("Could not read config file `{}` due to error `{}`. ", JSON_FILE_PATH, err);
            if let Err(e) = fs::write(path, DEFAULT_JSON){
                eprintln!("and could not create a default config file due to error `{}`. Below is the default.\n{}", e, DEFAULT_JSON);
            }else{eprintln!("so created a default config file.");}
            return Err("Error reading file.".to_string());
        });
        return Self::new_from_str(json, abs_path);
    }
    fn new_from_str(json: String, mut abs_path: PathBuf)->Result<Self, String>{
        let json = unwrap_or!(JSON::string_to_object(json), err=>return Err(
            format!("Error Parsing JSON. {:?}", err)
        ));

        let json = if let JSON::Dict(json) = json{json}else
        {return Err(format!("expected a dict as the root element"));};

        let path_prefix = if json.contains_key(PATH_PREFIX_KEY){
            if let JSON::String(prefix) = (**json.get(PATH_PREFIX_KEY).unwrap()).clone(){prefix}else
            {return Err(format!("expected a the key `{}` to have a string value.", PATH_PREFIX_KEY));}
        }else{String::new()};

        let output_path = if json.contains_key(OUTPUT_PATH_KEY){
            if let JSON::String(prefix) = (**json.get(OUTPUT_PATH_KEY).unwrap()).clone(){prefix}else
            {return Err(format!("expected a the key `{}` to have a string value.", OUTPUT_PATH_KEY));}
        }else{return Err(format!("expected the root to contain the key `{}`.", OUTPUT_PATH_KEY));};

        let relative_path = if json.contains_key(RELATIVE_PATH_KEY){
            if let JSON::String(path) = (**json.get(RELATIVE_PATH_KEY).unwrap()).clone(){path}else
            {return Err(format!("expected a the key `{}` to have a string value.", RELATIVE_PATH_KEY));}
        }else{".".to_string()};
        abs_path.push(relative_path);

        let mut out_path = abs_path.clone();
        out_path.push(output_path);

        return Ok(Self::new(path_prefix,out_path, abs_path));
    }
    fn new(path_prefix: String, output_path: PathBuf, dir_path: PathBuf)->Self{Self{path_prefix, output_path, dir_path}}
}
fn get_settings()->Result<Settings, ()>{
    let mut path = env::current_exe().expect("couldnt find the path of version-selector");
    path.pop();
    let mut json_path = path.clone();
    json_path.push(JSON_FILE_PATH);
    return Ok(unwrap_or_log_exit!(Settings::new_from_path(json_path, path), Err(())));
}

fn get_version_files(settings: &Settings) -> Result<Vec<fs::DirEntry>, String>{
    let exe_exception = env::current_exe().expect("couldnt find the path of version-selector");
    let json_excpetion = {
        let mut path = env::current_exe().expect("couldnt find the path of version-selector");
        path.pop();
        let mut json_path = path.clone();
        json_path.push(JSON_FILE_PATH);
        json_path
    };
    Ok(unwrap_or!(fs::read_dir(settings.dir_path.clone()), err=>return Err(
        format!("could not read items in dir due to error `{}`.", err)
    ))
    .map(|p|p.expect("couldnt access path"))
    .filter(|p|(
        p.file_name().to_str().expect("invalid file name").starts_with(&settings.path_prefix) && 
        p.path() != exe_exception &&
        p.path() != json_excpetion &&
        p.path() != settings.output_path
    ))
    .collect::<Vec<_>>())
}

struct Command{
    name: &'static str,
    help: &'static str,
    func: fn(Vec<String>),
}

macro_rules! commands {
    ($(#[$name:literal :$help: literal] |$val:ident| $body:expr ),*) => {
        [
            $(Command{
                name: $name,
                help: $help,
                // the function is called recurse as the only place this function name is valid in user code is from the function (i.e. if you are createing a recursive call) so the most sensible name for the user is recurse as that is what you will be doing when calling it
                func: {fn recurse($val: Vec<String>) { $body } recurse},
            },)*
        ]
    }
}

static COMMANDS: [Command; 6] = commands![
    #["--version": "this is the prints the version of the selector application"] |args|println!("{}",VERSION),
    #["help": "this is the help command"] |args|{
        if args.len() == 0{
            for command in COMMANDS.iter(){
                println!("{}: {}", command.name, command.help);
            }
            return;
        }else if args.len() == 1{
            for command in COMMANDS.iter(){
                if command.name == args[0]{
                    println!("{}: {}", command.name, command.help);
                    return;
                }
                println!("command {} does not exist", args[0]);
            }
        }else{eprintln!("Error help only accepts one optional argument");}
    },
    #["list": "lists all installed versions"] |args|{
        if args.len() > 0{
            eprintln!("Error list accepts no arguments");
            return;
        }
        let settings = unwrap_or_exit!(get_settings());
        for ver in unwrap_or_log_exit!(get_version_files(&settings)){
            println!("{}",ver.file_name().to_str().unwrap());
        }
    },
    #["select": "selects a version"] |args|{
        if args.len() > 1{
            eprintln!("Error select only accepts one optional argument");
            return;
        }
        let settings = unwrap_or_exit!(get_settings());
        let version_files = unwrap_or_log_exit!(get_version_files(&settings));
        let selected_version = if args.len() == 0{
            for (i, item) in version_files.iter().enumerate(){
                println!("{}: {}", i, item.file_name().to_str().unwrap());
            }
            print!("Choose Version: ");
            io::stdout().flush().expect("flush failed");
            let mut version = String::new();
            io::stdin()
            .read_line(&mut version)
            .expect("failed to read from stdin");
            let version = unwrap_or!(version.trim().parse::<usize>(), .. => {
                recurse(vec![version.trim().to_string()]);
                return;
            });
            version_files[version].path()
        }else{// use the arguments as the version
            let version_str = &args[0];
            let mut version = version_files.iter().find(|i|i.file_name().to_str().unwrap() == version_str);
            if version.is_none(){
                // do the non-prefixed checks after all the prefixed checks so if you
                // have the files `prefix-name` and `prefix-prefix-name` it is posible
                // to select `prefix-name` this is required as we have no guarantee
                // about the order of the version_files
                version = version_files.iter().find(
                    |i|i.file_name().to_str().unwrap() == settings.path_prefix.clone()+version_str
                );
            }
            let version = if version.is_none(){
                eprintln!("could not find version `{}` (note when running this command without a version argument it gives you an interactive list to select from)",version_str);
                return;
            }else{version.unwrap()};
            version.path()
        };
        if let Err(e) = symlink::create_symlink(&selected_version, &settings.output_path){
            if let symlink::ErrorKind::PathAlreadyExists(_) = e{
                if !unwrap_or!(settings.output_path.symlink_metadata(), e =>{
                    eprintln!("Error could not select version because read of the ouput file failed due to error `{}`", e);
                    return;
                }).is_symlink(){
                    eprintln!(
                        "Error output file exists and is not a symlink so you will loose data if you select a version. Please manualy rename or remove the file `{}`",
                        path::absolute(settings.output_path).unwrap().display()
                    );
                    return;
                }
                if let Err(e) = symlink::remove_symlink(&settings.output_path){
                    eprintln!("Error could not select version because removal of the previous version symlink failed due to error `{}`",e);
                    return;
                }
                if let Err(e) = symlink::create_symlink(&selected_version, &settings.output_path){
                    eprintln!("Error could not select version because symlink creation failed due to error `{}`", e);
                    return; 
                }
            }else{
                eprintln!("Error could not select version because symlink creation failed due to error `{}`", e);
                return;
            }
        }
        println!("SUCSESS. `{}` is now the selected version of the app.",path::absolute(selected_version).unwrap().display());
    },
    #["where": "alias to the which command"] |args| run_command("which",args),
    #["which": "shows the selected version"] |args|{
        if args.len() > 0{
            eprintln!("Error where accepts no arguments");
            return;
        }
        let settings = unwrap_or_exit!(get_settings());
        let target = unwrap_or!(settings.output_path.canonicalize(), _=>{
            println!("no selected version as the selected version path is invalid or does not exist");
            return;
        });
        println!("{}",target.display());
    }
];

fn run_command(cmd: &str, args: Vec<String>){
    for command in COMMANDS.iter(){
        if command.name == cmd{
            (command.func)(args);
            return;
        }
    }
    if cmd == "help"{panic!("no help command");}
    eprintln!("Could not find command {}\nBelow is the help:", cmd);
    run_command("help",Vec::new());
}

fn main() {
    if env::args().count() == 1{
        run_command("help", Vec::new());
        return;
    }
    if env::args().count() < 1{panic!("invalid argument count");}
    //the above garantee at least two arguments

    run_command(&env::args().nth(1).unwrap(), env::args().skip(2).collect());
}
