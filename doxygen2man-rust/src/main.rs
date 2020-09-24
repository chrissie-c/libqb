extern crate xml;

use structopt::StructOpt;
use std::fs::File;
use std::io::BufReader;
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

struct FnParam
{
    par_name: String,
    par_type: String,
}

struct FnStruct
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
    fn_structs: Vec<FnStruct>,
}
impl FunctionInfo {
    pub fn new() -> FunctionInfo {
        FunctionInfo {
            fn_type: "".to_string(),
            fn_name: "".to_string(),
            fn_argstring: "".to_string(),
            fn_brief: "".to_string(),
            fn_detail: "".to_string(),
            fn_args: Vec::<FnParam>::new(),
            fn_structs: Vec::<FnStruct>::new(),
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
    return "".to_string();
}


fn collect_text(parser: &mut EventReader<BufReader<File>>) -> String
{
    let mut text:String = "".to_string();
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
                                text += collect_text(parser).as_str();
                            }
                            "sp" => {
                                text += " ";
                            }
                            "parameternamelist" => {
                            }
                            "highlight" | "emphasis" => {
                                text += "\\fB";
                                text += collect_text(parser).as_str();
                                text += "\\fR";
                            }
                            "parametername" => {
                            }
                            "computeroutput" => {
                                text += "\n.nf\n";
                                text += collect_text(parser).as_str();
                                text += "\n.fi\n";
                            }
                            _ => {} // TODO MORE!
                        }
                        // CC: Check for things like <sp> <para> etc etc
//                        text += collect_text(parser).as_str();
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


fn TEST_print_function(f: &FunctionInfo)
{
    println!("FUNCTION {} {} {}", f.fn_type, f.fn_name, f.fn_argstring);
    for i in &f.fn_args {
        println!("  PARAM: {} {} ", i.par_type, i.par_name);
    }
    println!("BRIEF: {}", f.fn_brief);
    println!("DETAIL: {}", f.fn_detail);
    println!("----------------------");
}


fn collect_function_param(parser: &mut EventReader<BufReader<File>>) -> FnParam
{
    let mut par_name: String = "".to_string();
    let mut par_type: String = "".to_string();

    loop {
        let er = parser.next();
        match er {
            Ok(e) => {
                match &e {
                    XmlEvent::StartElement {name, ..} => {
                        let tmp = collect_text(parser).as_str().to_string();
//                        println!("Param element: {} {}", name, tmp);

                        if name.to_string() == "type" {
                            par_type = tmp.clone();
                        }
                        if name.to_string() == "declname" {
                            par_name = tmp.clone();
                        }
                    }
                    XmlEvent::EndElement {..} => {
                        return FnParam{par_name, par_type};
                    }
                    _e => { //println!("PARAM OTHER: {:?}", _e);
                    }
                }
            }
            Err(e) => {
                println!("Error:{}", e);
                return FnParam{par_name, par_type}; //CC: OK ???
            }
        }
    }
}

fn collect_function_info(parser: &mut EventReader<BufReader<File>>)
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
                                function.fn_type = collect_text(parser);
                            },
                            "definition" =>  {
                                let _fntext = collect_text(parser).as_str().to_string();
                            }
                            "argsstring" => {
                                function.fn_argstring = collect_text(parser);
                            }
                            "name" => {
                                function.fn_name = collect_text(parser);
                            }
                            "param" => {
                                function.fn_args.push(collect_function_param(parser))
                            }
                            "briefdescription" => {
                                function.fn_brief = collect_text(parser);
                            }
                            "detaileddescription" => {
                                function.fn_detail = collect_text(parser);
                            }
                            _ => {
                                // Not used,. but still need to consume it
                                let _fntext = collect_text(parser);
                            }
                        }
                    }
                    XmlEvent::Characters(_s) => {
                        //println!("Function Chars: {}", s); // CC: This is the actual text
                    }
                    XmlEvent::EndElement {..} => {
                        TEST_print_function(&function);
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

fn  read_file(parser: &mut EventReader<BufReader<File>>)
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
                            // go down the tree colelcting info until we ready End Element
                            collect_function_info( parser /* ,place to collect info */);
                        }
                    },
                    XmlEvent::EndElement {..} => {
                        //                println!("{}: End: {}", depth, name);
                    },
                    XmlEvent::Characters(_s) => {
                        //                println!("Chars: {}", s); // CC: This is the actual text
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

fn main() {

    let opt = Opt::from_args();
//    println!("{:?}", opt);

    let file = File::open(opt.xml_file).unwrap();
    let mut parser = ParserConfig::new()
        .whitespace_to_characters(true)
        .ignore_comments(true)
        .create_reader(BufReader::new(file));

    read_file(&mut parser);
}
