use std::{
    cmp::Ordering,
    error::Error,
    thread::sleep,
    time::Duration,
    fs::{
        self,
        File
    },
    io::{
        Read,
        Write
    },
    path::{
        self,
        PathBuf,
        Path
    },
    sync::OnceLock
};

//security:
use argon2::{
    password_hash::{phc::PasswordHash, PasswordVerifier},
    Argon2,
};
use rpassword::ConfigBuilder;

//self-update:
use self_update::cargo_crate_version;

use csv::WriterBuilder; //.csv creation

//decoding R&S UTF-16-Little-Endian encoding
use encoding_rs::UTF_16LE;
use encoding_rs_io::DecodeReaderBytesBuilder;

//plot creation
use plotters::prelude::*;
use plotters::style::text_anchor::{Pos, HPos, VPos};

//reading and writing the .ini
use serde::{Serialize, Deserialize};


static CONFIG: OnceLock<Config> = OnceLock::new(); //to store config file
//safety related:
const PASS_ACTUAL_HASH: &str = "$argon2id$v=19$m=19456,t=2,p=1$fIpUMMpCyhLhWblxaIhtpg$hcrlizZD/QtXfWJcmrQ9TLmWzyQgWSkHwVzkgUELQQw";
const SEC_CHECK: bool = true;
static PASS_USER_ATTEMPT: OnceLock<String> = OnceLock::new();
static SKIP_PASS: OnceLock<bool> = OnceLock::new();

//self-update
const REPO_OWNER: &str = "SebastianMolex";
const REPO_NAME: &str = "stitcher";
const BIN_NAME: &str = "Plotter";
const REPO_LINK: &str = "https://github.com/SebastianMolex/stitcher";


#[derive(Debug, Serialize, Deserialize)]
struct Config { //config file data
    test_dir: String,
    graph_output_dir_name: String,
    csvs_output_dir_name: String,

    test_auto_name: String,
    test_manual_name: String,
    test_imm_identifier_name: String,
    test_limit_folder: String,
    test_limit_extension: String,

    test_identifier_peak: String,
    test_identifier_average: String,
    test_identifier_quasipeak: String,
    test_identifier_additional_baselevel: String,

    test_oscilloscope_any_file: String,
    test_oscilloscope_lin_identifier: String,
    test_oscilloscope_log_identifier: String,
    test_oscilloscope_prefix: String,
    test_oscilloscope_middle: String,
    test_oscilloscope_suffix: String,
    test_oscilloscope_limit_lin: String,
    test_oscilloscope_limit_log: String,

    test_oscilloscope_graph_1_lin_name: String,
    test_oscilloscope_graph_1_log_name: String,
    test_oscilloscope_graph_2_lin_name: String,
    test_oscilloscope_graph_2_log_name: String,
    test_oscilloscope_graph_3_lin_name: String,
    test_oscilloscope_graph_3_log_name: String,
    test_oscilloscope_graph_4_lin_name: String,
    test_oscilloscope_graph_4_log_name: String,

    dim_x: i32,
    dim_y: i32,

    offset_y_top_emi: f64,
    offset_y_bot_emi: f64,
    offset_y_rounding_emi: f64,

    offset_y_bot_oscilloscope: f64,
    offset_y_top_oscilloscope: f64,
    offset_y_rounding_oscilloscope: f64,


    main_color: String,
    secondary_color: String,

    graph_color_1: String,
    graph_color_2: String,
    graph_color_3: String,
    graph_color_4: String,

    limit_color_pk: String,
    limit_color_av: String,
    limit_color_qp: String,
    limit_color_baselevel: String,

    style_general_type: String,
    style_general_size: i32,
    style_marker_type: String,
    style_marker_size: i32,

    chart_x_label_area_size: i32,
    chart_y_label_area_size: i32,
    chart_margin: i32,

    mesh_alpha: f64,
    mesh_thickness: u32,

    custom_ticks_length: i32,
    custom_ticks_gap: i32,

    legend_line_thickness: u32,

    marker_size: i32,
    marker_close_criterion_x: i32,
    marker_close_criterion_y: i32,
    marker_close_shift: i32,
    marker_prefix: String,
    marker_pos_offset_base: usize,
    marker_pos_offset_body: usize,
    marker_pos_offset_scale: usize,
    marker_pos_offset_line_base: i32,
    marker_pos_offset_line_scale: i32,
}

#[derive(Debug, PartialEq)]
struct RawData {
    classifiers: Vec<String>,
    types: Vec<(String, String)>,
    values: Vec<Vec<(f64, Option<f64>)>>,
}

#[derive(Debug, PartialEq)]
struct Limit {
    name: String,
    values: Vec<(f64, f64)>,
}

#[derive(Debug, PartialEq, Serialize)]
struct Peak {
    frequency: f64,
    level: f64,
    limit_name: String,
    limit_level: f64,
}

#[derive(PartialEq)]
enum TestType {
    Emi,
    Imm,
    VolvoOscilloscope,
}

#[derive(PartialEq)]
struct Test {
    typ: TestType,
    path: PathBuf,
}

fn verify_password(password: &str, stored_hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(stored_hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

fn update() -> Result<(), Box<dyn Error>> {
    let status = self_update::backends::github::Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name(BIN_NAME)
        .show_download_progress(true)
        .current_version(cargo_crate_version!())
        .build()?
        .update()?;
    println!("Update status: `{}`!", status.version());
    Ok(())
}

#[inline(always)]
fn safety_check() {
    SKIP_PASS.get_or_init(|| SEC_CHECK);
}

#[inline(always)]
fn pass_question() {
    if let Some(skip_pass) = SKIP_PASS.get() && *skip_pass {return;}
    println!("| Warning! | This Copy is Password Protected");
    let pass_conf = ConfigBuilder::new().password_feedback_mask('*').build();
    let attempt = rpassword::prompt_password_with_config("| Message! | Please enter the password: ", pass_conf);
    match attempt {
        Ok(v) => {
            PASS_USER_ATTEMPT.get_or_init(|| {v});
        },
        Err(_) => {
            println!("| Error! | could not read Password");
            sleep(Duration::from_secs(5));
            panic!();
        },
    }
}

#[inline(always)]
fn drm() {
    if let Some(skip_pass) = SKIP_PASS.get() && *skip_pass {return;}
    let attempt = PASS_USER_ATTEMPT.get();
    if let Some(v) = attempt{
        if !verify_password(v.trim(), PASS_ACTUAL_HASH) {
            println!("| Error! | Password is incorrect");
            sleep(Duration::from_secs(5));
            panic!();
        }
    } else {
        println!("| Error! | No Password found");
        sleep(Duration::from_secs(5));
        panic!();
    }
}

fn main() {
    match update() {
        Ok(_) => {
            println!("| Message! | Updated Successfully");
        },
        Err(_) => {
            println!("| Error! | Update could not be loaded, please download it manually: {}", REPO_LINK);
            sleep(Duration::from_secs(5));
            return;
        },
    }
    safety_check();
    pass_question();
    drm();
    if !Path::new("config.ini").exists() {//if config doesnt exist, create it
        println!("| Warning! | no config found, generating new config");
        match create_new_ini() {
            Ok(_) => {
                println!("| Message! | config generated");
            },
            Err(_) => {
                println!("| Error! | could not generate config");
                sleep(Duration::from_secs(5));
                return;
            },
        }
    }
    println!("| Message! | loading config...");
    get_config(); //loads config to global static
    drm();
    let conf = match CONFIG.get() {
        Some(v) => {v},
        None => {
            println!("| Error! | could not load config");
            sleep(Duration::from_secs(5));
            return;
        },
    };
    println!("| Message! | config found, generating plots...");
    //plotting:

    let mut tests_to_plot: Vec<Test> = vec![];
    let path = match path::absolute(&conf.test_dir) {
        Ok(v) => {v},
        Err(_) => {
            println!("| Error! | could not load test directory from config");
            sleep(Duration::from_secs(5));
            return;
        },
    };
    drm();
    match search_for_all_test_paths(&path, &mut tests_to_plot) { //finds all types of tests
        Ok(_) => {println!("| Message! | all plots scanned, begin plotting...");},
        Err(_) => {
            println!("| Error! | test path crawling failed");
            sleep(Duration::from_secs(5));
            return;
        },
    };

    for test in tests_to_plot {
        let dir = test.path;
        match test.typ {
            TestType::Emi => {
                match plotter_emi(&dir) {
                    Ok(_) => {},
                    Err(_) => {
                        println!("| Error! | could not generate EMI graph for {}", dir.to_string_lossy());
                        sleep(Duration::from_secs(5));
                        return;
                    },
                }
            },
            TestType::Imm => {
                match plotter_imm(&dir) {
                    Ok(_) => {},
                    Err(_) => {
                        println!("| Error! | could not generate Imm graph for {}", dir.to_string_lossy());
                        sleep(Duration::from_secs(5));
                        return;
                    },
                }
            },
            TestType::VolvoOscilloscope => {
                match plotter_volvo_oscilloscope(&dir) {
                    Ok(_) => {},
                    Err(_) => {
                        println!("| Error! | could not generate Oscilloscope graph for {}", dir.to_string_lossy());
                        sleep(Duration::from_secs(5));
                        return;
                    },
                }
            },
        }
    }

    println!("| Message! | all plots generated. Closing software...");
    sleep(std::time::Duration::from_secs(5));
}

fn get_config() -> &'static Config {
    CONFIG.get_or_init(|| {
        let content = match std::fs::read_to_string("config.ini") {
            Ok(v) => {v},
            Err(_) => {
                println!("| Error! | could not load config into global context");
                sleep(Duration::from_secs(5));
                panic!();
            },
        };
        match serde_ini::from_str(&content){
            Ok(v) => {v},
            Err(_) => {
                println!("| Error! | could not parse config");
                sleep(Duration::from_secs(5));
                panic!();
            },

        }
    })
}

fn create_new_ini() -> Result<(), Box<dyn Error>>{
    let config = Config { //default values
        test_dir: "../../../../Tests/".to_string(),
        graph_output_dir_name: "!raw_graph".to_string(),
        csvs_output_dir_name: "!csvs".to_string(),

        test_auto_name: "Preview Result 1.Result".to_string(),
        test_manual_name: "Result Table.Result".to_string(),
        test_imm_identifier_name: "Result Table_EutFailures.Result".to_string(),
        test_limit_folder: "System".to_string(),
        test_limit_extension: "LimitLine".to_string(),

        test_identifier_peak: "PK".to_string(),
        test_identifier_average: "AV".to_string(),
        test_identifier_quasipeak: "QP".to_string(),
        test_identifier_additional_baselevel: "Baselevel".to_string(),

        test_oscilloscope_any_file: "Graph_1_Lin.Wfm.csv".to_string(),
        test_oscilloscope_lin_identifier: "Lin".to_string(),
        test_oscilloscope_log_identifier: "Log".to_string(),
        test_oscilloscope_prefix: "Graph_".to_string(),
        test_oscilloscope_middle: "_".to_string(),
        test_oscilloscope_suffix: "".to_string(),
        test_oscilloscope_limit_lin: "Limit_Lin.csv".to_string(),
        test_oscilloscope_limit_log: "Limit_Log.csv".to_string(),

        test_oscilloscope_graph_1_lin_name: "L_Current_Lin".to_string(),
        test_oscilloscope_graph_1_log_name: "L_Current_Log".to_string(),
        test_oscilloscope_graph_2_lin_name: "CM_Current_Lin".to_string(),
        test_oscilloscope_graph_2_log_name: "CM_Current_Log".to_string(),
        test_oscilloscope_graph_3_lin_name: "DM_Voltage_Lin".to_string(),
        test_oscilloscope_graph_3_log_name: "DM_Voltage_Log".to_string(),
        test_oscilloscope_graph_4_lin_name: "CM_Voltage_Lin".to_string(),
        test_oscilloscope_graph_4_log_name: "CM_Voltage_Log".to_string(),

        dim_x: 1920,
        dim_y: 1080,
        offset_y_top_emi: 20.0,
        offset_y_bot_emi: 20.0,
        offset_y_rounding_emi: 10.0,

        offset_y_bot_oscilloscope: 1.0,
        offset_y_top_oscilloscope: 1.0,
        offset_y_rounding_oscilloscope: 1.0,
        main_color: "#FFFFFF".to_string(),
        secondary_color: "#000000".to_string(),
        graph_color_1: "#0000FF".to_string(),
        graph_color_2: "#000000".to_string(),
        graph_color_3: "#0000FF".to_string(),
        graph_color_4: "#00FF00".to_string(),
        limit_color_pk: "#00FF00".to_string(),
        limit_color_av: "#FF0000".to_string(),
        limit_color_qp: "#00FF00".to_string(),
        limit_color_baselevel: "#FF00FF".to_string(),
        style_general_type: "sans-serif".to_string(),
        style_general_size: 30,
        style_marker_type: "sans-serif".to_string(),
        style_marker_size: 22,
        chart_x_label_area_size: 75,
        chart_y_label_area_size: 100,
        chart_margin: 50,
        mesh_alpha: 0.3,
        mesh_thickness: 1,
        custom_ticks_length: 30,
        custom_ticks_gap: 4,
        legend_line_thickness: 3,
        marker_size: 6,
        marker_close_criterion_x: 20,
        marker_close_criterion_y: 20,
        marker_close_shift: 5,
        marker_prefix: "M".to_string(),
        marker_pos_offset_base: 160,
        marker_pos_offset_body: 40,
        marker_pos_offset_scale: 180,
        marker_pos_offset_line_base: 100,
        marker_pos_offset_line_scale: 10,
    };
    let ini_string = serde_ini::to_string(&config)?;
    fs::write("config.ini", ini_string)?;
    Ok(())
}

fn hex_to_rgb(hex: &str) -> Result<RGBColor, Box<dyn Error>> {
    let clean = hex.strip_prefix('#').unwrap_or(hex);
    if clean.len() != 6 {
        Err("Invalid hex length")?;
    }

    let val = u32::from_str_radix(clean, 16).map_err(|_| "Invalid hex digits")?;

    //minor bitmagic
    let r = u8::try_from((val >> 16) & 0xFF)?;
    let g = u8::try_from((val >> 8) & 0xFF)?;
    let b = u8::try_from(val & 0xFF)?;

    Ok(RGBColor(r, g, b))
}

fn read_utf16le_file(path: &str) -> Result<String, std::io::Error> { //used for emissions (and immunity?)
    let file = File::open(path)?;

    // Build a decoder that treats the input as UTF-16 LE
    let mut decoder = DecodeReaderBytesBuilder::new()
        .encoding(Some(UTF_16LE))
        .build(file);

    let mut contents = String::new();
    decoder.read_to_string(&mut contents)?;

    Ok(contents)
}

fn read_volvo_oscilloscope_file(path: &str) -> Result<String, std::io::Error> {
    fs::read_to_string(path)
}

//recursive folder search
//probably takes decades if given a root folder of a drive.
//this should only take the folder where tests are actually expected to be.
fn search_for_all_test_paths(base_dir: &PathBuf, result: &mut Vec<Test>) -> Result<(), Box<dyn Error>> { //emissions tests are all such tests, that have a "Preview Result 1.Result" file.
    let conf = CONFIG.get().ok_or("could not fetch config")?;
    if let Ok(dirs) = fs::read_dir(base_dir){
        for entry in dirs.flatten() {
            let path = entry.path();
            if path.is_dir() {
                search_for_all_test_paths(&path, result)?;
            } else {
                if let Some(file_name) = path.file_name() {
                    //establish type of test
                    let typ = match file_name.to_string_lossy().as_ref() {
                        v if v == conf.test_auto_name || v == conf.test_manual_name => {
                            //check if the immunity classic file exists:
                            let mut check_path = path.parent().ok_or("Could not establish folder of potential test")?.to_owned();
                            check_path.push(&conf.test_imm_identifier_name);
                            match check_path.try_exists() {
                                Ok(val) => {
                                    if val {
                                        TestType::Imm //it does exists
                                    } else {
                                        TestType::Emi //it does not exist
                                    }
                                },
                                Err(_) => {
                                    println!("| Error! | Insufficient Read permissions for {}", check_path.to_string_lossy());
                                    sleep(Duration::from_secs(5));
                                    panic!();
                                },
                            }

                        },
                        v if v == conf.test_oscilloscope_any_file => {TestType::VolvoOscilloscope},
                        _ => {continue;},
                    };

                    //found the file, add dir as a test if it does not already exist.
                    let test = Test {
                        typ,
                        path: path.parent().ok_or(format!("could not search given test path: {}", path.to_string_lossy()))?.to_path_buf(),
                    };

                    if !result.contains(&test){
                        result.push(test);
                    }

                } //given error occurs only if some clown places a result in a drive root and sets that as the Tests directory in the config.
            }     //actually even then it shouldn't trigger as the .parent() should resolve to the drive's root, no?
        }
    }
    Ok(())
}

fn get_raw_data_emi(file_path: &str) -> Result<RawData, Box<dyn Error>> { //(Vec<(Freq Unit: String, Level Unit: String)>, Vec<Vec<(Freq: f64, Level: f64)>>)
    let conf = CONFIG.get().ok_or("could not fetch config")?;
    let mut output_values = vec![];
    let mut output_types = vec![];
    let mut output_classifiers = vec![];
    //if preview results exists, it was an auto-test, else it is a manual test.
    let text = if Path::new(&(file_path.to_string() + "/" + &conf.test_auto_name)).exists() {
        read_utf16le_file(&(file_path.to_string() + "/" + &conf.test_auto_name))?
    } else {
        read_utf16le_file(&(file_path.to_string() + "/" + &conf.test_manual_name))?
    };

    let mut is_detector = vec![];
    let mut prefix_done = false;
    let mut unit_line = "".to_string(); //since the defining typeline that tells the meaning of this lines comes after if, it has to be stored until that line is encountered
    let mut classifier_line = "".to_string(); //since the defining typeline that tells the meaning of this lines comes after if, it has to be stored until that line is encountered

    for (row_id, line) in text.lines().enumerate() {
        if line == "[TableValues]" { //table begins, set flag so the next checks are skipped to save cycles.
            prefix_done = true;
            continue;
        } else if !prefix_done {
            if line.contains("Unit=") {//remember this line until the type_line is encountered
                unit_line = line.to_string();
            } else if line.contains("Name="){
                classifier_line = line.to_string();
            } else if line.contains("ColTyp=") {
                //line that explains unit_lines & classifier_lines meaning encountered, generate output_types now.
                let types_line_parts: Vec<&str> = line.split(['\t', ' ']).collect();
                let mut units_line_parts = unit_line.split(['\t', ' ']).skip(1);
                let mut classifier_line_parts = classifier_line.split(['\t', ' ']).skip(2);
                let mut freq_unit = "".to_string();

                for part in types_line_parts {
                    match part {
                        "Frequency" => {
                            freq_unit = units_line_parts.next().ok_or("could not fetch frequency unit")?.to_string();
                        },
                        "Level" => {
                            output_types.push((freq_unit.clone(), units_line_parts.next().ok_or("could not fetch level unit")?.to_string()));
                            output_classifiers.push(classifier_line_parts.next().ok_or("could not fetch classifier")?.to_string());
                        },
                        _ => {},
                    }
                }
                //finished generation of types
            } else if line.contains("Detector=") && !line.contains("Indicate") {
                //store which table rows are actual detectors in a list.
                let mut detector_parts: Vec<&str> = line.split(['\t', ' ']).collect();
                detector_parts.remove(0);
                for part in detector_parts {
                    if !part.is_empty() {
                        is_detector.push(part.parse::<i32>().is_ok_and(|n| n != 0)); // push true if elem is a non-zero number else push false
                    }
                }
            }
            continue;
        }
        //value table handling:

        //read line to output
        //let line = line.split(['L', 'N', 'V', 'H']).collect::<Vec<_>>().first().unwrap().to_owned(); //Too naive, current probe tests do not have this cuts off when L(1) or N is encountered in the table
        let mut line_parts: Vec<&str> = line.split(['\t', ' ']).collect();
        is_detector[0] = true; // re-add frequency-column to be kept

        let mut is_detector_clone = is_detector.clone();
        line_parts.retain(move |_| is_detector_clone.remove(0)); //remove everything that is not a detector



        //pre-create vectors
        if output_values.is_empty() {
            output_values = vec![Vec::new(); line_parts.len()-1]; //-1 since no extra place for frequency column
        }

        for i in 1..line_parts.len() {
            output_values[i-1].push( //-1 so index remains zero index'd despite loop starting at one.
                match (line_parts[0].parse::<f64>(), line_parts[i].parse::<f64>()) { //--- can still be in the data, indicating a gap in the plot. Mark this via None's
                    (Ok(freq), Ok(level)) => {(freq, Some(level))}, //nominal value
                    (Ok(freq), Err(_)) => {(freq, None)}, //gap
                    _ => {
                        println!("| Warning! | unexpected value encountered in {}; column: {}; row: {}", file_path, i, row_id);
                        continue;
                    } //should probably not happen
                }
            );
        }
    }
    Ok(RawData { classifiers: output_classifiers, types: output_types, values: output_values })
}

fn _get_raw_data_imm(_file_path: &str) -> Result<RawData, Box<dyn Error>> { //(Vec<(Freq Unit: String, Level Unit: String)>, Vec<Vec<(Freq: f64, Level: f64)>>)
    todo!();
}

fn get_raw_data_volvo_oscilloscope(file_path: &str) -> Result<RawData, Box<dyn Error>> { //(Vec<(Freq Unit: String, Level Unit: String)>, Vec<Vec<(Freq: f64, Level: f64)>>)
    let conf = CONFIG.get().ok_or("could not fetch config")?;
    let mut output_values = vec![];
    let mut output_types = vec![];
    //if preview results exists, it was an auto-test, else it is a manual test or an immunity one.
    let divs = [&conf.test_oscilloscope_lin_identifier, &conf.test_oscilloscope_log_identifier];
    let output_classifiers = vec![
        "Lin".to_string(), "Lin".to_string(), "Lin".to_string(), "Lin".to_string(),
        "Log".to_string(), "Log".to_string(), "Log".to_string(), "Log".to_string()
    ];

    for (did, div) in divs.into_iter().enumerate() {
        for column_id in 1..=4 {
            let header = read_volvo_oscilloscope_file(&(file_path.to_string() + "/" + &conf.test_oscilloscope_prefix + &column_id.to_string() + &conf.test_oscilloscope_middle + div + &conf.test_oscilloscope_suffix + ".csv"))?;
            let text = read_volvo_oscilloscope_file(&(file_path.to_string() + "/" + &conf.test_oscilloscope_prefix + &column_id.to_string() + &conf.test_oscilloscope_middle + div + &conf.test_oscilloscope_suffix + ".Wfm.csv"))?;

            //get units from the header
            let y_unit = if let Some(start) = header.find("BaseUnit:") {
                let sub_string = &header[start+9..]; //+9 to cut out the "BaseUnit:""
                if let Some(end) = sub_string.find(":") {
                    sub_string[..end].to_string()
                } else {
                    println!("| Error! | Header file does not match format: {}", file_path.to_string() + "/" + &conf.test_oscilloscope_prefix + &column_id.to_string() + &conf.test_oscilloscope_middle + div + &conf.test_oscilloscope_suffix + ".csv");
                    sleep(Duration::from_secs(5));
                    panic!();
                }
            } else {
                println!("| Error! | Header file does not match format: {}", file_path.to_string() + "/" + &conf.test_oscilloscope_prefix + &column_id.to_string() + &conf.test_oscilloscope_middle + div + &conf.test_oscilloscope_suffix + ".csv");
                sleep(Duration::from_secs(5));
                panic!();
            };
            if did == 0 {
                output_types.push(("s".to_string(), y_unit)); //too hard linked to value to extract string to config easily
            } else {
                output_types.push(("Hz".to_string(), y_unit)); //too hard linked to value to extract string to config easily
            }

            //now read the actual data

            for (row_id, line) in text.lines().enumerate() {
                //read line to output
                let line_parts: Vec<&str> = line.split([',']).collect();

                //pre-create vectors
                if output_values.is_empty() {
                    output_values = vec![Vec::new(); 8];
                }

                output_values[4 * did + (column_id - 1)].push(
                    match (line_parts[0].parse::<f64>(), line_parts[1].parse::<f64>()) { //--- can still be in the data, indicating a gap in the plot. Mark this via None's
                            (Ok(freq), Ok(level)) => {(freq.max(0.1), Some(level))}, //nominal value
                            (Ok(freq), Err(_)) => {(freq.max(0.1), None)}, //gap
                            _ => {
                                println!("| Warning! | unexpected value encountered in {}; row: {}", file_path, row_id);
                                continue;
                            } //should probably not happen
                        }
                );
            }
        }
    }
    Ok(RawData { classifiers: output_classifiers, types: output_types, values: output_values })
}

fn get_limits_emi(file_path: &str) -> Result<Vec<Limit>, Box<dyn Error>> {
    let conf = CONFIG.get().ok_or("could not fetch config")?;
    let mut output: Vec<Limit> = vec![];

    let file_paths: Vec<String> = get_files_with_extension(&(file_path.to_string() + "/" + &conf.test_limit_folder), &conf.test_limit_extension)?;

    for (j, file_path) in file_paths.into_iter().enumerate() {
        let text = read_utf16le_file(&file_path)?;
        let mut prefix_done = false;

        //get limit type from filename:
        let name = Path::new(&file_path).file_stem().and_then(|name| name.to_str()).ok_or("could not convert filepath to text")?.to_string();

        output.push(Limit { name, values: vec![] });

        let mut prev_line = "--- ---"; //simulate a gap at the start

        for curr_line in text.lines() {
            if curr_line == "[TableValues]" {
                prefix_done = true;
                continue;
            } else if !prefix_done {
                continue;
            }
            //read lines to output
            //i would need a diagramm to explain this algo...
            //limits are structured so, that they act like a list of coordinates that the path takes. If the path is interrupted a "gap" = ("---", "---") is encountered.
            //The plotter however requires a list of lines, not a path. In comes this algo:
            //imagine this algo like a moving group of two pointers (current and previous element) that adds both elements if both elements are not a gap.
            //this duplicates all elements not after or before a gap aka "Middles" get split into one "End" and one "Start" thus turning a path into a list of lines.
            //critical, that this algo does nothing if the input happens to already resemble a list of lines aka this algo is idempotent.
            let curr_line_parts: Vec<&str> = curr_line.split(['\t', ' ']).collect();
            let prev_line_parts: Vec<&str> = prev_line.split(['\t', ' ']).collect();

            if curr_line_parts[0] != "---" && curr_line_parts[1] != "---" && prev_line_parts[0] != "---" && prev_line_parts[1] != "---" {
                output[j].values.push((prev_line_parts[0].parse::<f64>()?, prev_line_parts[1].parse::<f64>()?));
                output[j].values.push((curr_line_parts[0].parse::<f64>()?, curr_line_parts[1].parse::<f64>()?));
            }
            prev_line = curr_line;
        }
    }

    Ok(output)
}

fn _get_limits_imm(_file_path: &str) -> Result<Vec<Limit>, Box<dyn Error>> {
    todo!()
}

fn get_limits_volvo_oscilloscope(file_path: &str) -> Result<Vec<Limit>, Box<dyn Error>> {
    let conf = CONFIG.get().ok_or("could not fetch config")?;
    let mut output: Vec<Limit> = vec![];

    let file_path_lin: String = file_path.to_string() + "/" + &conf.test_oscilloscope_limit_lin;
    let file_path_log: String = file_path.to_string() + "/" + &conf.test_oscilloscope_limit_log;

    let text_lin = read_volvo_oscilloscope_file(&file_path_lin)?;
    let text_log = read_volvo_oscilloscope_file(&file_path_log)?;


    output.push(Limit { name: conf.test_oscilloscope_graph_1_lin_name.clone(), values: vec![] });
    output.push(Limit { name: conf.test_oscilloscope_graph_1_log_name.clone(), values: vec![] });


    for line_lin in text_lin.lines() {
        let line_parts: Vec<&str> = line_lin.split([',']).collect();
        output[0].values.push((line_parts[0].parse::<f64>()?, line_parts[1].parse::<f64>()?));
    }

    for line_log in text_log.lines() {
        let line_parts: Vec<&str> = line_log.split([',']).collect();
        output[1].values.push((line_parts[0].parse::<f64>()?, line_parts[1].parse::<f64>()?));
    }

    Ok(output)
}

//uses double log-graph interpolation.
//currently unused, due to values always being linear, either directly of listed in dB.
/*fn peak_search_log_log(data: &[(f64, Option<f64>)], limit: &Limit, limit_start_index: usize, limit_end_index: usize) -> Option<Peak>{
    //for each frequency start/end point pair this finds the maximum value of data in this range then checks if that exceeds the limit.
    //only works if the limit is monotonic between start and end
    //if all are true adds it as a point to the output.
    let mut result: Option<((f64, f64), f64)> = None;

    let limit_start = limit.values[limit_start_index];
    let limit_end = limit.values[limit_end_index];


    for point in data {
        if point.0 < limit_start.0{continue;}
        if point.0 > limit_end.0{break;}
        //log-log-interpolation using binary log for fast calculation. formula doesnt differ for other log bases due to log properties.
        let interpolation_exponent = f64::log2(limit_end.1 / limit_start.1) / f64::log2(limit_end.0 / limit_start.0); //k = log(y2/y1)/log(x2/x1) ->positive if monotonically increasing, negative if monotonically decreasing.
        let interpolated_limit = limit_start.1 * (point.0 / limit_start.1).powf(interpolation_exponent); //y = y1 * (x/x1)^k
        if let Some(point_1) = point.1
            && point_1 >= interpolated_limit && point_1 > result.unwrap_or(((-1f64, f64::MIN), -1f64)).0.1{
                //breaks limit and is greater than last break
                result = Some(((point.0, point_1), interpolated_limit));
            }
    }

    result.map(|result| Peak {
            frequency: result.0.0,
            level: result.0.1,
            limit_name: limit.name.clone(),
            limit_level: result.1,
        }) //returns none if no value ever breaks the limit
}*/

fn peak_search_log_linear(data: &[(f64, Option<f64>)], limit: &Limit, limit_start_index: usize, limit_end_index: usize) -> Option<Peak>{
    //for each frequency start/end point pair this finds the maximum value of data in this range then checks if that exceeds the limit.
    //only works if the limit is monotonic between start and end
    //if all are true adds it as a point to the output.
    let mut result: Option<((f64, f64), f64)> = None;

    let limit_start = limit.values[limit_start_index];
    let limit_end = limit.values[limit_end_index];


    for point in data {
        if point.0 < limit_start.0{continue;}
        if point.0 > limit_end.0{break;}
        //log-lin-interpolation using binary log for fast calculation. formula doesnt differ for other log bases due to log properties.
        let x_factor = f64::log2(point.0 / limit_start.0) / f64::log2(limit_end.0 / limit_start.0);
        let interpolated_limit = limit_start.1 + (limit_end.1 - limit_start.1) * x_factor;
        if let Some(point_1) = point.1
            && point_1 >= interpolated_limit && point_1 > result.unwrap_or(((-1f64, f64::MIN), -1f64)).0.1{
                //breaks limit and is greater than last break
                result = Some(((point.0, point_1), interpolated_limit));
            }
    }

    result.map(|result| Peak {
            frequency: result.0.0,
            level: result.0.1,
            limit_name: limit.name.clone(),
            limit_level: result.1,
        }) //returns none if no value ever breaks the limit
}

fn build_color_lists(order: &[&str]) -> Result<(Vec<RGBColor>, Vec<RGBColor>), Box<dyn Error>> {
    let conf = CONFIG.get().ok_or("could not fetch config")?;
    let mut output1 = vec![];
    let mut output2 = vec![];

    for &item in order {
        match item {
            v if *v == conf.test_identifier_additional_baselevel => {
                output1.push(hex_to_rgb(&conf.graph_color_4)?);
                output2.push(hex_to_rgb(&conf.limit_color_baselevel)?);
            },
            v if *v == conf.test_identifier_quasipeak => {
                output1.push(hex_to_rgb(&conf.graph_color_3)?);
                output2.push(hex_to_rgb(&conf.limit_color_qp)?);
            },
            v if *v == conf.test_identifier_average => {
                output1.push(hex_to_rgb(&conf.graph_color_2)?);
                output2.push(hex_to_rgb(&conf.limit_color_av)?);
            },
            v if *v == conf.test_identifier_peak => {
                output1.push(hex_to_rgb(&conf.graph_color_1)?);
                output2.push(hex_to_rgb(&conf.limit_color_pk)?);
            },
            _ => {panic!("Found item that matches no pattern")},
        }
    }


    Ok((output1, output2))
}

fn plotter_emi(file_path: &Path) -> Result<(), Box<dyn Error>> {
    drm();
    let file_path = file_path.to_string_lossy();
    let file_path = &file_path.as_ref();

    let conf = CONFIG.get().ok_or("could not fetch config")?;

    //get raw_data and limits from test:
    //it is critical that raw_datas index matches the respective limits index. Sort logic goes through PK/AV/QP shortcut in limit name in /System of test.

    //extract testname from file_path
    let testname = Path::new(file_path).file_name().and_then(|name| name.to_str()).ok_or("could not convert filepath to text")?;

    let raw = get_raw_data_emi(file_path)?;
    let (classes, types, raw_data) = (raw.classifiers, raw.types, raw.values);
    let limits: Vec<Limit> = get_limits_emi(file_path)?;

    if raw_data.is_empty() { //if true most likely case: Preview Result 1.Result is in binary format, so quit
        println!("| Warning! | Cannot fetch raw data for {}. It is most likely still in binary format!", file_path);
        return Ok(()); //ok and not error, as this can just be ignored and should not force a stop, like an error would.
    }

    //go through limits and add PK, AV and/or QP to the stack if it exists as a limit at all.

    let mut order = vec![];

    for class in &classes {
        match class {
            name if name.contains(&conf.test_identifier_quasipeak) => {
                order.push(conf.test_identifier_quasipeak.as_ref())
            },
            name if name.contains(&conf.test_identifier_peak) => {
                order.push(conf.test_identifier_peak.as_ref())
            },
            name if name.contains(&conf.test_identifier_average) => {
                order.push(conf.test_identifier_average.as_ref())
            },
            _ => {},
        }
    }

    order.sort_by_key(|&elem| {
        match elem {
            v if *v == conf.test_identifier_peak => 0,
            v if *v == conf.test_identifier_average => 1,
            v if *v == conf.test_identifier_quasipeak => 2,
            _ => u8::MAX,
        }
    });


    //build color order:
    let (color_list_plots, color_list_limits) = build_color_lists(&order)?;

    if raw_data.len() > color_list_plots.len() + 1 {panic!()};

    let freqs_plot: Vec<f64> = raw_data[0].iter().map(|(first, _)| first.to_owned()).collect();

    let f_min = freqs_plot.first().ok_or("could not load first frequency")?.to_owned();
    let f_max = freqs_plot.last().ok_or("could not load last frequency")?.to_owned();

    let (mut y_min, mut y_max) = find_min_max(&raw_data, &limits).ok_or("could not find minimum and maximum of data")?;

    y_min = ((y_min - conf.offset_y_bot_emi)/conf.offset_y_rounding_emi).round() * conf.offset_y_rounding_emi;
    y_max = ((y_max + conf.offset_y_top_emi)/conf.offset_y_rounding_emi).round() * conf.offset_y_rounding_emi;

    let bg_color = hex_to_rgb(&conf.main_color)?;
    let secondary_color = hex_to_rgb(&conf.secondary_color)?;
    let font_style = (conf.style_general_type.as_str(), conf.style_general_size).into_font();
    let font_style_marker = (conf.style_marker_type.as_str(), conf.style_marker_size).into_font();

    if conf.graph_output_dir_name.is_empty(){panic!()};
    if conf.csvs_output_dir_name.is_empty(){panic!()};

    //fs::create_dir_all(file_path.to_string() + "/" + &conf.graph_output_dir_name)?;
    //fs::create_dir_all(file_path.to_string() + "/" + &conf.csvs_output_dir_name)?;

    //for each trace make a graphic:

    for (idx, emi) in raw_data.into_iter().enumerate() {

        generate_raw_data_table(file_path, &conf.csvs_output_dir_name, testname, order[idx], &emi, &types[idx])?;

        //begin plotting
        //let specific_path = file_path.to_string() + &format!("/{}/{}_{}.svg", conf.output_dir_name, &testname, &order[idx]);
        //optional pulling out of the graphs to a more central location:
        let specific_path = crawl_up_to_tests(file_path, &conf.graph_output_dir_name, testname, &(order[idx].to_string() + ".svg"))?;

        fs::create_dir_all(Path::new(&specific_path).parent().ok_or("could not get root folder of graph destination")?.to_string_lossy().into_owned())?;
        println!("| Message! | Generating EMI Plot: {specific_path}");

        //create plot
        let root = SVGBackend::new(&specific_path, (conf.dim_x.try_into()?, conf.dim_y.try_into()?)).into_drawing_area();
        root.fill(&bg_color)?;

        let mut chart = ChartBuilder::on(&root)
            .caption(format!("{} {}", testname, &order[idx]), font_style.clone().color(&secondary_color))
            .x_label_area_size(conf.chart_x_label_area_size)
            .y_label_area_size(conf.chart_y_label_area_size)
            .margin(conf.chart_margin)
            .build_cartesian_2d((f_min .. f_max).log_scale(), y_min..y_max)?;

        chart.configure_mesh()
        .x_desc(format!("Frequency / {}", types[idx].0))
        .y_desc(format!("Emissions / {}", types[idx].1))
        .axis_desc_style(font_style.clone().with_color(secondary_color))
        .bold_line_style(secondary_color.mix(conf.mesh_alpha).stroke_width(conf.mesh_thickness))
        .light_line_style(secondary_color.mix(conf.mesh_alpha).stroke_width(conf.mesh_thickness))
        .disable_x_mesh() //only way to not render minor tickmark lines... sadly also disables major lines... gotta do em manually then...
        .disable_y_mesh()
        .y_max_light_lines(0)
        .x_max_light_lines(0)
        .label_style(TextStyle::from(font_style.clone()).with_color(secondary_color))
        .x_labels(5)
        .draw()?;

        //add crosses
        /*chart.draw_series(emi.iter().map(|&(x, y)| {
            EmptyElement::at((x, y)) + Cross::new((0, 0), 5, color_list_plots[idx])
        }))?;*/

        //add lined major gridlines:
        let pow_min = f_min.log10().floor() as i32;
        let pow_max = f_max.log10().ceil()  as i32;
        for pow in pow_min..pow_max {
            let x = 10f64.powi(pow);
            if x < f_min || x > f_max {
                continue;
            }
            chart.draw_series(
                LineSeries::new(
                    vec![(x, y_min), (x, y_max)],
                    secondary_color.mix(conf.mesh_alpha)
                )
            )?;
        }


        //add dotted minor gridlines:
        for x in log_minor_lines(f_min, f_max, 8) {  // 8 → shows 2,3,…,9 per decade
            chart.draw_series(
                DottedLineSeries::new(
                    vec![(x, y_min), (x, y_max)],
                    0,
                    10,
                    move |c| Circle::new(c, 1, secondary_color.mix(conf.mesh_alpha).filled()),
                )
            )?;
        }

        //same for y-axis:

        let mut y = y_min;
        while y < y_max {
            y += 10.0;
            if y.abs() % 20.0 > 0.3 {
                chart.draw_series(DottedLineSeries::new(
                    vec![(f_min, y), (f_max, y)],
                    0,
                    10,
                    move |c| Circle::new(c, 1, secondary_color.mix(conf.mesh_alpha).filled()),
                ))?;
            } else {
                chart.draw_series(
                    LineSeries::new(
                        vec![(f_min, y), (f_max, y)],
                        secondary_color.mix(conf.mesh_alpha)
                    )
                )?;
            }
        }

        //and now the labels:

        let plotting_area = chart.plotting_area();

        let (px_left, py_bottom) = plotting_area.map_coordinate(&(f_min, y_min));
        let (px_right, _)        = plotting_area.map_coordinate(&(f_max, y_min));
        let tick_len = conf.custom_ticks_length;
        let gap = conf.custom_ticks_gap;

        //bottom left extra-tickmark

        if !is_power_of_ten(f_min) || (f_min - 0.1 < 0.0001) {
            root.draw(&PathElement::new(
                vec![(px_left, py_bottom), (px_left, py_bottom + tick_len)],
                secondary_color,
            ))?;
            let left_style = TextStyle::from(font_style.clone()).pos(Pos::new(HPos::Left, VPos::Top));
            root.draw_text(&format!("{:.2}", f_min), &left_style, (px_left, py_bottom + tick_len + gap))?;
        }

        //bottom right extra-tickmark

        if !is_power_of_ten(f_max) {
            root.draw(&PathElement::new(
                vec![(px_right, py_bottom), (px_right, py_bottom + tick_len)],
                secondary_color,
            ))?;
            let right_style = TextStyle::from(font_style.clone()).pos(Pos::new(HPos::Right, VPos::Top));
            root.draw_text(&format!("{:.2}", f_max), &right_style, (px_right, py_bottom + tick_len + gap))?;
        }


        //chart actual data the cloning of the data could be avoided if the peak_search was adjusted to run once at the beginning
        //that would double the looping over limits. Its speed vs space and this configuration focuses more on speed.
        let mut emi_split: Vec<Vec<(f64, f64)>> = Vec::new();
        let mut current: Vec<(f64, f64)> = Vec::new();

        for (freq, level_opt) in emi.iter() {
            if let Some(level) = level_opt {
                current.push((*freq, *level));
            } else if !current.is_empty() {
                emi_split.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            emi_split.push(current);
        }
        let mut temp = None;
        for emi_graph in emi_split {
            temp = Some(chart.draw_series(LineSeries::new(emi_graph.clone(), &color_list_plots[idx]))?);
        }
        if let Some(val) = temp {
            val.label(order[idx].to_string() + " Measurement Results").legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], color_list_plots[idx].stroke_width(conf.legend_line_thickness)));
        }



        let mut peaks: Vec<Peak> = vec![];

        for lim in limits.iter() { //for each limit
            let limit_name = &lim.name;
            let limit = &lim.values;
            if limit_name.contains(order[idx]) { //check if the limit_name should be plotted
                for i in 0..(limit.len()/2) {
                    if limit[2*i+1].0 <= f_min {continue;} //dont plot sub-limits that end before the graph has even begun
                    if limit[2*i].0 >= f_max {break;} //dont plot sub-limits that start where the graph is already over

                    let peak = peak_search_log_linear(&emi, lim, 2*i, 2*i+1);
                    //plot the peaks above the limits
                    if let Some(peak) = peak {
                        //push peak to peak table
                        peaks.push(peak);
                    }

                    //plot each limit chunk(breaks if the limit pair format start<->finish is broken):
                    let temp = chart.draw_series(LineSeries::new(vec![limit[2*i], limit[2*i+1]], color_list_limits[idx].stroke_width(3)))?;
                    if i == 0 {
                        //and add it to the legend once
                        let color_clone = color_list_limits[idx];
                        temp.label(limit_name).legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], color_clone.stroke_width(3)));
                    }
                }
            }
        }

        //and finally plot the limit breaks. cannot be done before, because all peaks need to be known at start so close ones can be considered for positioning.
        //first sort peaks by their frequency
        peaks.sort_by(|a, b| a.frequency.total_cmp(&b.frequency));
        for i in 0..peaks.len() {
            let (ppx, ppy) = if i != 0 {chart.plotting_area().map_coordinate(&(peaks[i-1].frequency, peaks[i-1].level))} else {(i32::MIN, i32::MIN)}; //previous
            let (mut pcx, pcy) = chart.plotting_area().map_coordinate(&(peaks[i].frequency, peaks[i].level)); //current
            let (pnx, pny) = if i != peaks.len() - 1 {chart.plotting_area().map_coordinate(&(peaks[i+1].frequency, peaks[i+1].level))} else {(i32::MIN, i32::MIN)}; //previous

            //plotting the markers themselves (triangles)

            //triangle marker properties:
            let half_width: i32 = conf.marker_size/2;
            let height: i32 = conf.marker_size;

            let vertices = vec![
                (pcx, pcy),                       // tip (bottom)
                (pcx - half_width, pcy - height), // top-left
                (pcx + half_width, pcy - height), // top-right
                (pcx, pcy),                       // close path
            ];

            let triangle = Polygon::new(vertices, secondary_color);

            root.draw(&triangle)?;

            //the ordeal to position the number next to the triangle with (hopefully) little overlap
            let style = if ppx != i32::MIN && (pcx - ppx) < conf.marker_close_criterion_x && (pcy - ppy) < conf.marker_close_criterion_y {
                if pnx != i32::MIN && (pnx - pcx) < conf.marker_close_criterion_x && (pny - pcy) < conf.marker_close_criterion_y {
                    //both too close -> centered, they will be offset
                    TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Center, VPos::Bottom))
                } else {
                    //only previous too close -> align-right
                    //check if this would push it out of the screen
                    /*
                    if pcx < (120 + chart.plotting_area().dim_in_pixel().0) as i32 {
                        pcx += 5;
                        TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Left, VPos::Bottom))
                    } else {
                        TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Center, VPos::Bottom))
                    }
                    */
                    //looks worse than if it just extends beyond the plot
                    pcx += conf.marker_close_shift;
                    TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Left, VPos::Bottom))
                }
            } else {
                if pnx != i32::MIN && (pnx - pcx) < conf.marker_close_criterion_x && (pny - pcy) < conf.marker_close_criterion_y {
                    //only next too close -> align-left
                    //check if this would push it out of the screen
                    /*
                    if pcx > 20 {
                        pcx -= 5;
                        TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Right, VPos::Bottom))
                    } else {
                        TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Center, VPos::Bottom))
                    }
                    */
                    //looks worse than if it just extends beyond the plot
                    pcx -= conf.marker_close_shift;
                    TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Right, VPos::Bottom))
                } else {
                    //neither too close -> centered
                    TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Center, VPos::Bottom))
                }
            };

            root.draw_text(
                &format!("{}{}", conf.marker_prefix, i+1),
                &style,
                (pcx, pcy - height)
            )?;


            //now just place the peak info starting in the top left of the image
            let style = TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Left, VPos::Top));
            let text_number = Text::new(format!("{}{})", conf.marker_prefix, i + 1), ((conf.marker_pos_offset_base + 180 * i) as i32, conf.marker_pos_offset_line_base + conf.marker_pos_offset_line_scale), &style);
            let text_freq = Text::new(format!("{:.2} {}", peaks[i].frequency, types[idx].0), ((conf.marker_pos_offset_base + conf.marker_pos_offset_body + conf.marker_pos_offset_scale * i) as i32, conf.marker_pos_offset_line_base), &style);
            let text_level = Text::new(format!("{:.2} {}", peaks[i].level, types[idx].1), ((conf.marker_pos_offset_base + conf.marker_pos_offset_body + conf.marker_pos_offset_scale * i) as i32, conf.marker_pos_offset_line_base + 2 * conf.marker_pos_offset_line_scale), &style);

            root.draw(&text_number)?;
            root.draw(&text_freq)?;
            root.draw(&text_level)?;

        }

        generate_peak_table(file_path, &conf.csvs_output_dir_name, testname, order[idx], peaks, &types[idx])?;

        let chart_area = chart.plotting_area();

        chart_area.draw(&Rectangle::new(
            [(f_min, y_min), (f_max, y_max)],
            ShapeStyle::from(&secondary_color).stroke_width(2)
        ))?;

        //position legend correctly
        chart.configure_series_labels()
            .position(SeriesLabelPosition::LowerRight)
            .label_font(font_style.clone())
            .border_style(secondary_color)
            .background_style(bg_color)
            .draw()?;



        root.present()?;

    }



    Ok(())
}

fn plotter_imm(file_path: &Path) -> Result<(), Box<dyn Error>> {
    drm();
    Err(format!("| Warning! | Immunity Plotting not yet implemented. Test: {}", file_path.to_string_lossy()))?
}

fn plotter_volvo_oscilloscope(file_path: &Path) -> Result<(), Box<dyn Error>> {
    let file_path = file_path.to_string_lossy();
    let file_path = &file_path.as_ref();

    let conf = CONFIG.get().ok_or("could not fetch config")?;

    //get raw_data and limits from test:
    //it is critical that raw_datas index matches the respective limits index. Sort logic goes through PK/AV/QP shortcut in limit name in /System of test.

    //extract testname from file_path
    let testname = Path::new(file_path).file_name().and_then(|name| name.to_str()).ok_or("could not convert filepath to text")?;

    let raw = get_raw_data_volvo_oscilloscope(file_path)?;
    let (types, raw_data) = (raw.types, raw.values);
    let limits: Vec<Limit> = get_limits_volvo_oscilloscope(file_path)?;

    if raw_data.is_empty() { //if true most likely case: Preview Result 1.Result is in binary format, so quit
        println!("| Warning! | Cannot fetch raw data for {}", file_path);
        return Ok(()); //ok and not error, as this can just be ignored and should not force a stop, like an error would.
    }


    //plot pre-setup

    let order = [
        &conf.test_oscilloscope_graph_1_lin_name, &conf.test_oscilloscope_graph_2_lin_name, &conf.test_oscilloscope_graph_3_lin_name, &conf.test_oscilloscope_graph_4_lin_name,
        &conf.test_oscilloscope_graph_1_log_name, &conf.test_oscilloscope_graph_2_log_name, &conf.test_oscilloscope_graph_3_log_name, &conf.test_oscilloscope_graph_4_log_name
    ];
    let colors_plot = [
        hex_to_rgb(&conf.graph_color_1)?, hex_to_rgb(&conf.graph_color_1)?, hex_to_rgb(&conf.graph_color_2)?, hex_to_rgb(&conf.graph_color_2)?,
        hex_to_rgb(&conf.graph_color_1)?, hex_to_rgb(&conf.graph_color_1)?, hex_to_rgb(&conf.graph_color_2)?, hex_to_rgb(&conf.graph_color_2)?
        ];
    let color_limit = hex_to_rgb(&conf.limit_color_pk)?;

    if raw_data.len() > colors_plot.len() + 1 {panic!()};

    let lin_len = limits[0].values.len();
    let log_len = limits[1].values.len();
    let f_lin_low: f64 = limits[0].values[0].0;
    let f_log_low: f64 = limits[1].values[0].0;
    let f_lin_high: f64 = limits[0].values[lin_len - 1].0;
    let f_log_high: f64 = limits[1].values[log_len - 1].0;

    let f_min = f_lin_low.min(f_log_low);
    let f_max = f_lin_high.max(f_log_high);

    let (mut y_min, mut y_max) = find_min_max(&raw_data, &limits).ok_or("could not find minimum and maximum of data")?;

    y_min = ((y_min - conf.offset_y_bot_oscilloscope)/conf.offset_y_rounding_oscilloscope).round() * conf.offset_y_rounding_oscilloscope;
    y_max = ((y_max + conf.offset_y_top_oscilloscope)/conf.offset_y_rounding_oscilloscope).round() * conf.offset_y_rounding_oscilloscope;

    let bg_color = hex_to_rgb(&conf.main_color)?;
    let secondary_color = hex_to_rgb(&conf.secondary_color)?;
    let font_style = (conf.style_general_type.as_str(), conf.style_general_size).into_font();
    let font_style_marker = (conf.style_marker_type.as_str(), conf.style_marker_size).into_font();

    if conf.graph_output_dir_name.is_empty(){panic!()};
    if conf.csvs_output_dir_name.is_empty(){panic!()};

    //fs::create_dir_all(file_path.to_string() + "/" + &conf.graph_output_dir_name)?;
    //fs::create_dir_all(file_path.to_string() + "/" + &conf.csvs_output_dir_name)?;

    //for each trace make a graphic:

    for (idx, datum) in raw_data.into_iter().enumerate() {

        generate_raw_data_table(file_path, &conf.csvs_output_dir_name, testname, order[idx], &datum, &types[idx])?;

        //begin plotting
        //let specific_path = file_path.to_string() + &format!("/{}/{}_{}.svg", conf.output_dir_name, &testname, &order[idx]);
        //optional pulling out of the graphs to a more central location:
        let specific_path = crawl_up_to_tests(file_path, &conf.graph_output_dir_name, testname, &(order[idx].to_string() + ".svg"))?;

        fs::create_dir_all(Path::new(&specific_path).parent().ok_or("could not get root folder of graph destination")?.to_string_lossy().into_owned())?;
        println!("| Message! | Generating Oscilloscope Plot: {specific_path}");

        //create plot
        let root = SVGBackend::new(&specific_path, (conf.dim_x.try_into()?, conf.dim_y.try_into()?)).into_drawing_area();
        root.fill(&bg_color)?;

        let mut chart = ChartBuilder::on(&root)
            .caption(format!("{} {}", testname, &order[idx]), font_style.clone().color(&secondary_color))
            .x_label_area_size(conf.chart_x_label_area_size)
            .y_label_area_size(conf.chart_y_label_area_size)
            .margin(conf.chart_margin)
            .build_cartesian_2d((f_min .. f_max).log_scale(), y_min..y_max)?;

        chart.configure_mesh()
        .x_desc(format!("{} / {}", (if order[idx].contains(&conf.test_oscilloscope_lin_identifier) {"Time"} else {"Frequency"}), types[idx].0))
        .y_desc(format!("Value / {}", types[idx].1))
        .axis_desc_style(font_style.clone().with_color(secondary_color))
        .bold_line_style(secondary_color.mix(conf.mesh_alpha).stroke_width(conf.mesh_thickness))
        .light_line_style(secondary_color.mix(conf.mesh_alpha).stroke_width(conf.mesh_thickness))
        .disable_x_mesh() //only way to not render minor tickmark lines... sadly also disables major lines... gotta do em manually then...
        .y_max_light_lines(0)
        .x_max_light_lines(0)
        .label_style(TextStyle::from(font_style.clone()).with_color(secondary_color))
        .x_labels(5)
        .draw()?;

        //add crosses
        /*chart.draw_series(datum.iter().map(|&(x, y)| {
            EmptyElement::at((x, y)) + Cross::new((0, 0), 5, colors_plot[idx])
        }))?;*/

        //add lined major gridlines:
        let pow_min = f_min.log10().floor() as i32;
        let pow_max = f_max.log10().ceil()  as i32;
        for pow in pow_min..pow_max {
            let x = 10f64.powi(pow);
            if x < f_min || x > f_max {
                continue;
            }
            chart.draw_series(
                LineSeries::new(
                    vec![(x, y_min), (x, y_max)],
                    secondary_color.mix(conf.mesh_alpha)
                )
            )?;
        }


        //add dotted minor gridlines:
        for x in log_minor_lines(f_min, f_max, 8) {  // 8 → shows 2,3,…,9 per decade
            chart.draw_series(
                DottedLineSeries::new(
                    vec![(x, y_min), (x, y_max)],
                    0,
                    10,
                    move |c| Circle::new(c, 1, secondary_color.mix(conf.mesh_alpha).filled()),
                )
            )?;
        }

        //and now the labels:

        let plotting_area = chart.plotting_area();

        let (px_left, py_bottom) = plotting_area.map_coordinate(&(f_min, y_min));
        let (px_right, _)        = plotting_area.map_coordinate(&(f_max, y_min));
        let tick_len = conf.custom_ticks_length;
        let gap = conf.custom_ticks_gap;

        //bottom left extra-tickmark

        if !is_power_of_ten(f_min) || (f_min - 0.1 < 0.0001) {
            root.draw(&PathElement::new(
                vec![(px_left, py_bottom), (px_left, py_bottom + tick_len)],
                secondary_color,
            ))?;
            let left_style = TextStyle::from(font_style.clone()).pos(Pos::new(HPos::Left, VPos::Top));
            root.draw_text(&format!("{:.2}", f_min), &left_style, (px_left, py_bottom + tick_len + gap))?;
        }

        //bottom right extra-tickmark

        if !is_power_of_ten(f_max) {
            root.draw(&PathElement::new(
                vec![(px_right, py_bottom), (px_right, py_bottom + tick_len)],
                secondary_color,
            ))?;
            let right_style = TextStyle::from(font_style.clone()).pos(Pos::new(HPos::Right, VPos::Top));
            root.draw_text(&format!("{:.2}", f_max), &right_style, (px_right, py_bottom + tick_len + gap))?;
        }

        let mut y = y_min;
        while y < y_max {
            y += 10.0;
            if y % 20.0 > 0.3 {
                chart.draw_series(DottedLineSeries::new(
                    vec![(f_min, y), (f_max, y)],
                    0,
                    10,
                    move |c| Circle::new(c, 1, secondary_color.mix(conf.mesh_alpha).filled()),
                ))?;
            }
        }

        //chart actual data the cloning of the data could be avoided if the peak_search was adjusted to run once at the beginning
        //that would double the looping over limits. Its speed vs space and this configuration focuses more on speed.
        let mut datum_split: Vec<Vec<(f64, f64)>> = Vec::new();
        let mut current: Vec<(f64, f64)> = Vec::new();

        for (freq, level_opt) in datum.iter() {
            if let Some(level) = level_opt {
                current.push((*freq, *level));
            } else if !current.is_empty() {
                datum_split.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            datum_split.push(current);
        }
        let mut temp = None;
        for datum_graph in datum_split {
            temp = Some(chart.draw_series(LineSeries::new(datum_graph.clone(), &colors_plot[idx]))?);
        }
        if let Some(val) = temp {
            val.label(order[idx].to_string() + " Measurement Results").legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], colors_plot[idx].stroke_width(conf.legend_line_thickness)));
        }



        let mut peaks: Vec<Peak> = vec![];

        for lim in &limits { //for each limit
            let limit_name = &lim.name;
            let limit = &lim.values;
            if order[idx].contains(limit_name) { //reversed from emi method
                for i in 0..(limit.len()/2) {
                    if limit[2*i+1].0 <= f_min {continue;} //dont plot sub-limits that end before the graph has even begun
                    if limit[2*i].0 >= f_max {break;} //dont plot sub-limits that start where the graph is already over

                    let peak = peak_search_log_linear(&datum, lim, 2*i, 2*i+1);
                    //plot the peaks above the limits
                    if let Some(peak) = peak {
                        //push peak to peak table
                        peaks.push(peak);
                    }

                    //plot each limit chunk(breaks if the limit pair format start<->finish is broken):
                    let temp = chart.draw_series(LineSeries::new(vec![limit[2*i], limit[2*i+1]], color_limit.stroke_width(3)))?;
                    if i == 0 {
                        //and add it to the legend once
                        let color_clone = color_limit;
                        temp.label(limit_name).legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], color_clone.stroke_width(3)));
                    }
                }
            }
        }

        //and finally plot the limit breaks. cannot be done before, because all peaks need to be known at start so close ones can be considered for positioning.
        //first sort peaks by their frequency
        peaks.sort_by(|a, b| a.frequency.total_cmp(&b.frequency));
        for i in 0..peaks.len() {
            let (ppx, ppy) = if i != 0 {chart.plotting_area().map_coordinate(&(peaks[i-1].frequency, peaks[i-1].level))} else {(i32::MIN, i32::MIN)}; //previous
            let (mut pcx, pcy) = chart.plotting_area().map_coordinate(&(peaks[i].frequency, peaks[i].level)); //current
            let (pnx, pny) = if i != peaks.len() - 1 {chart.plotting_area().map_coordinate(&(peaks[i+1].frequency, peaks[i+1].level))} else {(i32::MIN, i32::MIN)}; //previous

            //plotting the markers themselves (triangles)

            //triangle marker properties:
            let half_width: i32 = conf.marker_size/2;
            let height: i32 = conf.marker_size;

            let vertices = vec![
                (pcx, pcy),                       // tip (bottom)
                (pcx - half_width, pcy - height), // top-left
                (pcx + half_width, pcy - height), // top-right
                (pcx, pcy),                       // close path
            ];

            let triangle = Polygon::new(vertices, secondary_color);

            root.draw(&triangle)?;

            //the ordeal to position the number next to the triangle with (hopefully) little overlap
            let style = if ppx != i32::MIN && (pcx - ppx) < conf.marker_close_criterion_x && (pcy - ppy) < conf.marker_close_criterion_y {
                if pnx != i32::MIN && (pnx - pcx) < conf.marker_close_criterion_x && (pny - pcy) < conf.marker_close_criterion_y {
                    //both too close -> centered, they will be offset
                    TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Center, VPos::Bottom))
                } else {
                    //only previous too close -> align-right
                    //check if this would push it out of the screen
                    /*
                    if pcx < (120 + chart.plotting_area().dim_in_pixel().0) as i32 {
                        pcx += 5;
                        TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Left, VPos::Bottom))
                    } else {
                        TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Center, VPos::Bottom))
                    }
                    */
                    //looks worse than if it just extends beyond the plot
                    pcx += conf.marker_close_shift;
                    TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Left, VPos::Bottom))
                }
            } else {
                if pnx != i32::MIN && (pnx - pcx) < conf.marker_close_criterion_x && (pny - pcy) < conf.marker_close_criterion_y {
                    //only next too close -> align-left
                    //check if this would push it out of the screen
                    /*
                    if pcx > 20 {
                        pcx -= 5;
                        TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Right, VPos::Bottom))
                    } else {
                        TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Center, VPos::Bottom))
                    }
                    */
                    //looks worse than if it just extends beyond the plot
                    pcx -= conf.marker_close_shift;
                    TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Right, VPos::Bottom))
                } else {
                    //neither too close -> centered
                    TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Center, VPos::Bottom))
                }
            };

            root.draw_text(
                &format!("{}{}", conf.marker_prefix, i+1),
                &style,
                (pcx, pcy - height)
            )?;


            //now just place the peak info starting in the top left of the image
            let style = TextStyle::from(font_style_marker.clone()).pos(Pos::new(HPos::Left, VPos::Top));
            let text_number = Text::new(format!("{}{})", conf.marker_prefix, i + 1), ((conf.marker_pos_offset_base + 180 * i) as i32, conf.marker_pos_offset_line_base + conf.marker_pos_offset_line_scale), &style);
            let text_freq = Text::new(format!("{:.2} {}", peaks[i].frequency, types[idx].0), ((conf.marker_pos_offset_base + conf.marker_pos_offset_body + conf.marker_pos_offset_scale * i) as i32, conf.marker_pos_offset_line_base), &style);
            let text_level = Text::new(format!("{:.2} {}", peaks[i].level, types[idx].1), ((conf.marker_pos_offset_base + conf.marker_pos_offset_body + conf.marker_pos_offset_scale * i) as i32, conf.marker_pos_offset_line_base + 2 * conf.marker_pos_offset_line_scale), &style);

            root.draw(&text_number)?;
            root.draw(&text_freq)?;
            root.draw(&text_level)?;

        }

        generate_peak_table(file_path, &conf.csvs_output_dir_name, testname, order[idx], peaks, &types[idx])?;

        let chart_area = chart.plotting_area();

        chart_area.draw(&Rectangle::new(
            [(f_min, y_min), (f_max, y_max)],
            ShapeStyle::from(&secondary_color).stroke_width(2)
        ))?;

        //position legend correctly
        chart.configure_series_labels()
            .position(SeriesLabelPosition::LowerRight)
            .label_font(font_style.clone())
            .border_style(secondary_color)
            .background_style(bg_color)
            .draw()?;



        drm();
        root.present()?;

    }



    Ok(())
}

/*fn generate_raw_data_table(file_path: &str, data: &Vec<(f64, Option<f64>)>, types: &(String, String)) -> Result<(), Box<dyn Error>> {

    let mut file = File::create(file_path)?;
    file.write_all(&[0xEF, 0xBB, 0xBF])?; //UTF-8 BOM

    let mut wtr = WriterBuilder::new().from_writer(file);

    wtr.serialize("frequency,value")?;
    for datum in data {
        if let Some(datum_1) = datum.1 {
            wtr.serialize(format!("{:.2} {},{:.2} {}",datum.0, &types.0, datum_1, &types.1))?;
        }
    }
    wtr.flush()?;
    Ok(())
}*/

fn crawl_up_to_tests(file_path: &str, dir_name: &str, test_name: &str, specifier: &str) -> Result<String, Box<dyn Error>> {
    let mut output = vec![];
    let path = Path::new(file_path);
    let mut current = path.ancestors();
    let base;

    loop{
        match current.next() {
            Some(v) => {
                let name = match v.file_name() {
                    Some(val) => {val},
                    None => {
                        println!("| Error! | could not read folder name during crawl");
                        sleep(Duration::from_secs(5));
                        panic!();
                    },
                };
                if name == "Tests"{
                    base = current.next();
                    break;
                }
                output.push(name.to_string_lossy().to_string());
            },
            None => {return Err("no \"tests\" folder found during crawl")?},
        }
    }
    if let Some(base) = base {
        output.remove(0);
        let mut base = base.to_string_lossy().to_string();
        base += "\\Tests\\";
        match output.pop() {
            Some(v) => {base += &v;},
            None => {
                base = file_path.to_string(); //Tests has been found but no sub-folders between that an this test? -> its a test in Tests root so reset path
            },
        }

        base += "\\";
        base += dir_name;

        while let Some(dir) = output.pop() {
            base += "\\";
            base += &dir;
        }

        base += "\\";
        base += test_name;
        base += "_";
        base += specifier;
        Ok(base)
    } else {
        Err("could not get root after crawl")?
    }

}

fn generate_raw_data_table(file_path: &str, dir_name: &str, test_name: &str, specifier: &str, data: &Vec<(f64, Option<f64>)>, types: &(String, String)) -> Result<(), Box<dyn Error>> {

    let file_name = crawl_up_to_tests(file_path, dir_name, test_name, &(specifier.to_string() + "_Values.csv"))?;
    fs::create_dir_all(Path::new(&file_name).parent().ok_or("could not get root folder of raw_data csv destination")?.to_string_lossy().into_owned())?;
    let mut file = File::create(&file_name)?;

    file.write_all(&[0xEF, 0xBB, 0xBF])?; //UTF-8 BOM

    let mut wtr = WriterBuilder::new().from_writer(file);

    wtr.serialize("frequency,value")?;
    for datum in data {
        if let Some(datum_1) = datum.1 {
            wtr.serialize(format!("{:.2} {},{:.2} {}",datum.0, &types.0, datum_1, &types.1))?;
        }
    }
    wtr.flush()?;
    Ok(())
}

fn generate_peak_table(file_path: &str, dir_name: &str, test_name: &str, specifier: &str, peaks: Vec<Peak>, types: &(String, String)) -> Result<(), Box<dyn Error>> {


    let file_name = crawl_up_to_tests(file_path, dir_name, test_name, &(specifier.to_string() + "_Limit_Breaks.csv"))?;
    fs::create_dir_all(Path::new(&file_name).parent().ok_or("could not get root folder of peak_data csv destination")?.to_string_lossy().into_owned())?;
    let mut file = File::create(&file_name)?;

    file.write_all(&[0xEF, 0xBB, 0xBF])?; //UTF-8 BOM

    let mut wtr = WriterBuilder::new().from_writer(file);

    wtr.serialize("limit_name,frequency,value,limit_value")?;
    for peak in &peaks {
        wtr.serialize(format!("{},{:.2} {},{:.2} {},{:.2} {}", peak.limit_name, peak.frequency, types.0, peak.level, types.1, peak.limit_level, types.1))?;
    }
    wtr.flush()?;
    Ok(())
}

/*fn generate_peak_table(file_path: &str, peaks: Vec<Peak>, types: &(String, String)) -> Result<(), Box<dyn Error>> {

    let mut file = File::create(file_path)?;
    file.write_all(&[0xEF, 0xBB, 0xBF])?; //UTF-8 BOM

    let mut wtr = WriterBuilder::new().from_writer(file);

    wtr.serialize("limit_name,frequency,value,limit_value")?;
    for peak in &peaks {
        wtr.serialize(format!("{},{:.2} {},{:.2} {},{:.2} {}", peak.limit_name, peak.frequency, types.0, peak.level, types.1, peak.limit_level, types.1))?;
    }
    wtr.flush()?;
    Ok(())
}*/

fn log_minor_lines(min: f64, max: f64, max_light_lines: usize) -> Vec<f64> {
    let mut result = Vec::new();
    if min <= 0.0 || max <= 0.0 { return result; }

    let start_exp = min.log10().floor() as i32;
    let end_exp   = max.log10().ceil()  as i32;

    for exp in start_exp..=end_exp {
        let base = 10f64.powi(exp);
        for m in 1..=(max_light_lines + 1) {  // +1 because m=1 is the major tick
            let val = (m as f64) * base;
            if val > min && val < max && m != 1 {
                result.push(val);
            }
        }
    }
    result
}

fn get_files_with_extension(dir_path: &str, extension: &str) -> Result<Vec<String>, std::io::Error> {
    let entries = fs::read_dir(dir_path)?;

    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        // Check if it is a file and has the correct extension
        if path.is_file()
            && let Some(ext) = path.extension()
                && ext == extension {
                    files.push(path);
        }

    }
    let output = files.iter().map(|v|
        match v.to_str() {
            Some(v) => {v.to_owned()},
            None => {
                println!("| Error! | could not convert filename to text");
                sleep(Duration::from_secs(5));
                panic!();
            },
        }
    ).collect();
    Ok(output)
}

fn find_min_max(data: &[Vec<(f64, Option<f64>)>], limits: &[Limit]) -> Option<(f64, f64)> {

    let data: Vec<Vec<f64>> = data.iter().map(|inner|
        inner.iter().filter_map(|(_, val)|
            val.to_owned()
        ).collect::<Vec<f64>>()
    ).collect();

    let limits: Vec<Vec<f64>> = limits.iter().map(|inner|
        inner.values.iter().map(|(_, val)|
            val.to_owned()
        ).collect::<Vec<f64>>()
    ).collect();

    let (min_data, max_data) = data.iter()
        .flatten()
        .fold(None, |acc: Option<(f64, f64)>, &x| {
            Some(match acc {
                None => (x, x),
                Some((lo, hi)) => (
                    if x.total_cmp(&lo) == Ordering::Less { x } else { lo },
                    if x.total_cmp(&hi) == Ordering::Greater { x } else { hi },
                )
            })
        })?;

    let (min_lim, max_lim) = limits.iter()
    .flatten()
    .fold(None, |acc: Option<(f64, f64)>, &x| {
        Some(match acc {
            None => (x, x),
            Some((lo, hi)) => (
                if x.total_cmp(&lo) == Ordering::Less { x } else { lo },
                if x.total_cmp(&hi) == Ordering::Greater { x } else { hi },
            )
        })
    })?;

    Some((min_data.min(min_lim), max_data.max(max_lim)))
}

fn is_power_of_ten(n: f64) -> bool {
    if n <= 0.0 {
        return false;
    }
    let log10_x = n.log10();
    (log10_x - log10_x.round()).abs() < 1e-6
}