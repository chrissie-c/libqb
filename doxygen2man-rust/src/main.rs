extern crate xml;

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Error};
use std::fmt::Write;
use structopt::StructOpt;
use xml::reader::{EventReader, XmlEvent, ParserConfig};

#[derive(Debug, StructOpt)]
#[structopt(name = "doxygen2man2", about = "Convert doxygen files to man pages")]
/// This is a tool to generate API manpages from a doxygen-annotated header file.
/// First run doxygen on the file and then run this program against the main XML file
/// it created and the directory containing the ancilliary files. It will then
/// output a lot of *.3 man page files which you can then ship with your library.
///
/// You will need to invoke this program once for each .h file in your library,
/// using the name of the generated .xml file. This file will usually be called
/// something like <include-file>_8h.xml, eg qbipcs_8h.xml
///
/// If you want HTML output then simpy use nroff on the generated files as you
/// would do with any other man page.
///
struct Opt {
    #[structopt (short="a", long="print-ascii", help="Print ASCII dump of manpage to stdout")]
    print_ascii: bool,

    #[structopt (short="m", long="print-man", help="Write man page files to <output-dir>")]
    print_man: bool,

    #[structopt (short="P", long="print-params", help="print PARAMS section")]
    print_params: bool,

    #[structopt (short="g", long="print-general", help="Print general man page for the whole header file")]
    print_general: bool,

    #[structopt (short="q", long="quiet", help="Run quietly, no progress info printed")]
    quiet: bool,

    #[structopt (short="c", long="use-header-copyright", help="Use the Copyright date from the header file (if one can be found)")]
    use_header_copyright: bool,

    #[structopt (short="I", long="headerfile", default_value="unknown.h", help="Set include filename (default taken from XML)")]
    headerfile: String,

    #[structopt (short="i", long="header-prefix", default_value="", help="prefix for includefile. eg qb/")]
    header_prefix: String,

    #[structopt (short="s", long="section", default_value="3", help="write man pages into section <section>")]
    man_section: u32,

    #[structopt (short="S", long="start-year", default_value="2010", help="Start year to print at end of copyright line")]
    start_year: u32,

    #[structopt (short="d", long="xml-dir", default_value="./xml/", help="Directory for XML files")]
    xml_dir: String,

    #[structopt (short="D", long="manpage-date", default_value="2010", help="Date to print at top of man pages (format not checked)")]
    manpage_date: String,

    #[structopt (short="Y", long="manpage-year", default_value="2010", help="Year to print at end of copyright line")]
    manpage_year: u32,

    #[structopt (short="p", long="package-name", default_value="Package", help="Name of package for these man pages")]
    package_name: String,

    #[structopt (short="H", long="header-name", default_value="Programmer's Manual", help="Header text")]
    header: String,

    #[structopt (short="o", long="output_dir", default_value="./", help="Write all man pages to <dir>")]
    output_dir: String,

    #[structopt (short="O", long="header_src_dir", default_value="./", help="Directory for the original header files (often needed by -c above)")]
    header_src_dir: String,

// Positional parameters
     xml_file: String,
}

#[derive(Clone)]
struct FnParam
{
    par_name: String,
    par_type: String,
    par_refid: Option<String>,
}

#[derive(Clone)]
struct StructureInfo
{
    str_name: String,
    str_members: Vec<FnParam>,
}

struct FunctionInfo
{
    fn_type: String,
    fn_name: String,
    fn_argstring: String,
    fn_brief: String,
    fn_detail: String,
    fn_args: Vec<FnParam>,
    fn_refids: Vec<String>, // refids used in the function
}

impl FunctionInfo {
    pub fn new() -> FunctionInfo {
        FunctionInfo {
            fn_type: String::new(),
            fn_name: String::new(),
            fn_argstring: String::new(),
            fn_brief: String::new(),
            fn_detail: String::new(),
            fn_args: Vec::<FnParam>::new(),
            fn_refids: Vec::<String>::new(),
        }
    }
}


fn get_attr(e: &XmlEvent, attrname: &str) -> String
{
    match e {
        XmlEvent::StartElement {attributes,.. } => {
            for a in attributes {
                if a.name.to_string() == attrname {
                    return a.value.to_string();
                }
            }
        }
        _ => {}
    }
    return String::new();
}

fn collect_ref(parser: &mut EventReader<BufReader<File>>) -> String
{
    let mut text = String::new();

    loop {
        let er = parser.next();
        match er {
            Ok(e) => {
                match &e {
                    XmlEvent::StartElement {name, attributes, ..} => {
                        println!("REF element: {}: {:?}", name, attributes);
                        match name.to_string().as_str() {
                            "declname" => {
                                // THIS NEVER HAPPENS
                            }
                            _ => {}
                        }
                    }
                    XmlEvent::Characters(s) => {
//                        println!("REF characters: {}", s);
                        text += s;
                    }
                    XmlEvent::EndElement {..} => {
                        return text;
                    }
                    _ => {}
                }
            }
            Err(_s) => {}
        }
    }
}


fn collect_text(parser: &mut EventReader<BufReader<File>>, structures: &mut HashMap<String, StructureInfo>) -> String
{
    let mut text = String::new();
    loop {
        let er = parser.next();
        match er {
            Ok(e) => {
                match &e {
//                    XmlEvent::StartElement {name, attributes, ..} => {
                    XmlEvent::StartElement {name, ..} => {
//                        println!("text element: {} {:?}", name, attributes);
                        match name.to_string().as_str() {
                            "para" => {
                                text += "\n";
                                text += collect_text(parser, structures).as_str();
                            }
                            "sp" => {
                                text += " ";
                            }
                            "parameternamelist" => {
                            }
                            "highlight" | "emphasis" => {
                                text += "\\fB";
                                text += collect_text(parser, structures).as_str();
                                text += "\\fR";
                            }
                            "parametername" => {
                            }
                            "computeroutput" => {
                                text += "\n.nf\n";
                                text += collect_text(parser, structures).as_str();
                                text += "\n.fi\n";
                            }
                            "ref" => {
                                let refid = get_attr(&e, "refid");
                                let kind = get_attr(&e, "kindref");
                                let tmp = collect_ref(parser);
                                text += &tmp;

                                // If it's a struct, add it to the list to collect
                                // TODO: Check if this also includes enums
                                if kind == "compound" {
                                    match structures.get(&refid) {
                                        Some(_) => {} // It's already in here
                                        None => {
                                            let new_struct = StructureInfo {str_name: tmp.clone(), str_members: Vec::<FnParam>::new()};
                                            structures.insert(refid, new_struct);
                                        }
                                    }
                                }
                            }

                            _ => {} // TODO MORE!
                        }
                    }
                    XmlEvent::Characters(s) => {
                        text += s;
                    }
                    XmlEvent::EndElement {..} => {
                        return text;
                    }
                    _ => {}
                }
            }
            Err(e) => {
                println!("Error:{}", e);
                return text;
            }
        }
    }
}

// A stripped-down version of collect_text that also returns the refid
fn collect_param_text(parser: &mut EventReader<BufReader<File>>, structures: &mut HashMap<String, StructureInfo>) -> (String, String)
{
    let mut text = String::new();
    let mut refid = String::new();
    loop {
        let er = parser.next();
        match er {
            Ok(e) => {
                match &e {
                    XmlEvent::StartElement {name, ..} => {
                        match name.to_string().as_str() {
                            "ref" => {
                                refid = get_attr(&e, "refid");
                                let kind = get_attr(&e, "kindref");
                                let tmp = collect_ref(parser);
                                text += &tmp;

                                // If it's a struct, add it to the list to collect
                                // TODO: Check if this also includes enums
                                if kind == "compound" {
                                    match structures.get(&refid) {
                                        Some(_) => {} // It's already in here
                                        None => {
                                            let new_struct = StructureInfo {str_name: tmp.clone(), str_members: Vec::<FnParam>::new()};
                                            structures.insert(refid.clone(), new_struct);
                                        }
                                    }
                                }
                            }

                            _ => {} // TODO MORE!
                        }
                    }
                    XmlEvent::Characters(s) => {
                        text += s;
                    }
                    XmlEvent::EndElement {..} => {
                        return (text, refid);
                    }
                    _ => {}
                }
            }
            Err(e) => {
                println!("Error:{}", e);
                return (text, refid);
            }
        }
    }
}


fn TEST_print_function(f: &FunctionInfo)
{
    println!("FUNCTION {} {} {}", f.fn_type, f.fn_name, f.fn_argstring);
    for i in &f.fn_args {
        match &i.par_refid {
            Some(r) =>
                println!("  PARAM: {} {} ({})", i.par_type, i.par_name, r),
            None =>
                println!("  PARAM: {} {}", i.par_type, i.par_name),
        }
    }
    println!("BRIEF: {}", f.fn_brief);
    println!("DETAIL: {}", f.fn_detail);
    println!("----------------------");
}


fn collect_function_param(parser: &mut EventReader<BufReader<File>>,
                          structures: &mut HashMap<String, StructureInfo>) -> FnParam
{
    let mut par_name = String::new();
    let mut par_type = String::new();
    let mut par_refid = None;

    loop {
        let er = parser.next();
        match er {
            Ok(e) => {
                match &e {
                    XmlEvent::StartElement {name, ..} => {
                        let (tmp, refid) = collect_param_text(parser, structures);

                        if name.to_string() == "type" {
                            par_type = tmp.clone();
                            par_refid = Some(refid);
                        }
                        if name.to_string() == "declname" {
                            par_name = tmp.clone();
                        }
                    }
                    XmlEvent::EndElement {..} => {
                        return FnParam{par_name, par_type, par_refid};
                    }
                    _e => {
                    }
                }
            }
            Err(e) => {
                println!("Error:{}", e);
                return FnParam{par_name, par_type, par_refid: None}; //CC: OK ???
            }
        }
    }
}

fn collect_function_info(parser: &mut EventReader<BufReader<File>>,
                         functions: &mut Vec<FunctionInfo>,
                         structures: &mut HashMap<String, StructureInfo>)
{
    let mut function = FunctionInfo::new();

    loop {
        let er = parser.next();
        match er {
            Ok(e) => {
                match &e {
                    XmlEvent::StartElement {name, ..} => {
//                        println!("Function element: {} {:?}", name, attributes);

                        match name.to_string().as_str() {
                            "type" => {
                                function.fn_type = collect_text(parser, structures);
                            },
                            "definition" =>  {
                                let _fntext = collect_text(parser, structures).as_str().to_string();
                            }
                            "argsstring" => {
                                function.fn_argstring = collect_text(parser, structures);
                            }
                            "name" => {
                                function.fn_name = collect_text(parser, structures);
                            }
                            "param" => {
                                let param = collect_function_param(parser, structures);
                                // If the param has a refid then make a note of it so we
                                // can expand structures in the manpage
                                match &param.par_refid {
                                    Some(r) => function.fn_refids.push(r.clone()),
                                    None => {}
                                }
                                function.fn_args.push(param);
                            }
                            "briefdescription" => {
                                function.fn_brief = collect_text(parser, structures);
                            }
                            "detaileddescription" => {
                                function.fn_detail = collect_text(parser, structures);
                            }
                            _ => {
                                // Not used,. but still need to consume it
                                let _fntext = collect_text(parser, structures);
                            }
                        }
                    }
                    XmlEvent::Characters(_s) => {
                        //println!("Function Chars: {}", s); // CC: This is the actual text
                    }
                    XmlEvent::EndElement {..} => {
                        functions.push(function);
                        return;
                    }
                    _ => {}
                }
            }
            Err(e) => {
                println!("Error:{}", e);
                return;
            }
        }
   }
}

fn read_file(parser: &mut EventReader<BufReader<File>>,
             _opt: &Opt,
             functions: &mut Vec<FunctionInfo>,
             structures: &mut HashMap<String, StructureInfo>)
{
    loop {
        let er = parser.next();
        match er {
            Ok(e) => {
                match &e {
                    XmlEvent::StartElement {name, ..} => {
//                        println!("Start: {} {:?}", name, attributes);
                        if name.to_string() == "memberdef" && get_attr(&e, "kind") == "function" {
                            // Do function stuff
                            // go down the tree collecting info until we read EndElement
                            collect_function_info(parser,
                                                  functions,
                                                  structures);
                        }

                        // TODO header-general info
                        if name.to_string() == "compounddef" && get_attr(&e, "kind") == "file" {
                            let header_text = collect_text(parser, structures);
                            println!("HEADER TEXT: {}", header_text);
                        }
                    },
                    XmlEvent::EndElement {..} => {
                    },
                    XmlEvent::Characters(_s) => {
                        // println!("Chars: {}", s);
                    },
                    XmlEvent::EndDocument => return,
                    _ => {}
                }
            }
            Err(e) => {
                println!("Error:{}", e);
                return;
            }
        }
    }
}

// Read a single structure file
fn read_structure_file(parser: &mut EventReader<BufReader<File>>,
                       sinfo: &mut StructureInfo) -> Result<u32, Error>
{
    // TODO!

    return Ok(0);
}


// Read all the strcutre files we need for our functions
fn read_structures_files(opt: &Opt,
                         structures: &HashMap<String, StructureInfo>,
                         filled_structures: &mut HashMap<String, StructureInfo>)
{
    for (refid, s) in structures {
        println!("need to read STRUCT {} refid:  {}", s.str_name, refid);

        let mut xml_file = String::new();
        match write!(xml_file, "{}/{}.xml", &opt.xml_dir, &refid) {
            Ok(_f) => {}
            Err(e) => {
                println!("Error making structure XML file name for {}: {}", refid, e);
                return;
            }
        }

        match File::open(&xml_file) {
            Ok(f) => {

                let mut parser = ParserConfig::new()
                    .whitespace_to_characters(true)
                    .ignore_comments(true)
                    .create_reader(BufReader::new(f));

                let mut new_s = s.clone();
                println!("OPENED {}", xml_file);

                match read_structure_file(&mut parser, &mut new_s) {
                    Ok(_i) => {
                        // Add to the new map
                        filled_structures.insert(refid.clone(), new_s);
                    }
                    Err(_e) => {}
                }
            }
            Err(e) =>
                println!("Error, Cannot open structure file {}: {}", xml_file, e)
        }
    }
}

fn print_man_pages(_opt: &Opt,
                   functions: &Vec<FunctionInfo>,
                   _structures: &HashMap<String, StructureInfo>)
{
// Just a test ATM
    for f in functions {
        TEST_print_function(&f);
    }
}


fn main() {

    // Get command-line options
    let opt = Opt::from_args();

    let mut main_xml_file = String::new();

    match  write!(main_xml_file, "{}/{}", &opt.xml_dir, &opt.xml_file) {
        Ok(_f) => {}
        Err(e) => {
            println!("Error making main XML file name for {}: {}", opt.xml_file, e);
            return;
        }
    }

    match File::open(&main_xml_file) {
        Ok(f) => {
            let mut parser = ParserConfig::new()
                .whitespace_to_characters(true)
                .ignore_comments(true)
                .create_reader(BufReader::new(f));

            let mut functions = Vec::<FunctionInfo>::new();
            let mut structures = HashMap::<String, StructureInfo>::new();

            // Read it all into structures
            read_file(&mut parser, &opt, &mut functions, &mut structures);

            // Go through structures Map and read those files in
            let mut filled_structures = HashMap::<String, StructureInfo>::new();
            read_structures_files(&opt, &structures,
                                  &mut filled_structures);

            // Then print man pages!
            print_man_pages(&opt, &functions, &filled_structures);
        }
        Err(e) => {
            println!("Cannot open XML file {}: {}", &main_xml_file, e);
        }
    }
}
