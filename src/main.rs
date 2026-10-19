use std::{
    cmp::Ordering,
    error::Error,
    ffi::OsStr,
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

use csv::WriterBuilder;

use encoding_rs::UTF_16LE;
use encoding_rs_io::DecodeReaderBytesBuilder;

use plotters::prelude::*;
use plotters::style::text_anchor::{Pos, HPos, VPos};

use serde::{Serialize, Deserialize};


static CONFIG: OnceLock<Config> = OnceLock::new(); //to store config file

#[derive(Debug, Serialize, Deserialize)]
struct Config { //config file data
    test_dir: String,
    output_dir_name: String,

    dim_x: i32,
    dim_y: i32,

    offset_y_top: f64,
    offset_y_bot: f64,

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
    types: Vec<(String, String)>,
    values: Vec<Vec<(f64, f64)>>,
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



fn main() {
    if !Path::new("config.ini").exists() {
        println!("| Warning! | no config found, generating new config");
        create_new_ini().unwrap();
        println!("| Message! | config generated, terminating... relaunch to generate plot files!");
        sleep(Duration::from_secs(5));
        return; //halts execution
    } //if config doesnt exist, create it
    println!("| Message! | loading config...");
    get_config(); //loads config to global static
    let conf = CONFIG.get().unwrap();
    println!("| Message! | config found, generating plots...");
    //plotting:

    let mut paths_to_plot: Vec<PathBuf> = vec![];
    let path = path::absolute(&conf.test_dir).unwrap();
    search_for_all_test_paths(&path, &mut paths_to_plot);

    for dir in paths_to_plot {
        plotter(&dir).unwrap();
    }

    println!("| Message! | all plots generated without error. terminating...");
    sleep(std::time::Duration::from_secs(5));
}

fn get_config() -> &'static Config {
    CONFIG.get_or_init(|| {
        let content = std::fs::read_to_string("config.ini").unwrap();
        serde_ini::from_str(&content).unwrap()
    })
}

fn create_new_ini() -> Result<(), Box<dyn Error>>{
    let config = Config { //default values
        test_dir: "../../../../Tests/".to_string(),
        output_dir_name: "!raw_graph".to_string(),
        dim_x: 1920,
        dim_y: 1080,
        offset_y_top: 20.0,
        offset_y_bot: 20.0,
        main_color: "#FFFFFF".to_string(),
        secondary_color: "#000000".to_string(),
        graph_color_1: "#0000FF".to_string(),
        graph_color_2: "#000000".to_string(),
        graph_color_3: "#FF00FF".to_string(),
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

fn read_utf16le_file(path: &str) -> Result<String, std::io::Error> {
    let file = File::open(path)?;

    // Build a decoder that treats the input as UTF-16 LE
    let mut decoder = DecodeReaderBytesBuilder::new()
        .encoding(Some(UTF_16LE))
        .build(file);

    let mut contents = String::new();
    decoder.read_to_string(&mut contents)?;

    Ok(contents)
}

//recursive folder search
//probably takes decades if given a root folder of a drive.
//this should only take the folder where tests are actually expected to be.
fn search_for_all_test_paths(base_dir: &PathBuf, result: &mut Vec<PathBuf>) { //emissions tests are all such tests, that have a "Preview Result 1.Result" file.
    if let Ok(dirs) = fs::read_dir(base_dir){
        for entry in dirs.flatten() {
            let path = entry.path();
            if path.is_dir() {
                search_for_all_test_paths(&path, result);
            } else {
                if let Some(file_name) = path.file_name() && file_name  == OsStr::new("Preview Result 1.Result") {
                    //found the file, add dir as a test.
                    result.push(path.parent().unwrap().to_path_buf());
                }
            }
        }
    }
}

fn get_raw_data(file_path: &str) -> RawData { //(Vec<(Freq Unit: String, Level Unit: String)>, Vec<Vec<(Freq: f64, Level: f64)>>)
    let mut output_values = vec![];
    let mut output_types = vec![];
    let text = read_utf16le_file(&(file_path.to_string() + "/Preview Result 1.Result")).unwrap();
    let mut is_detector = vec![];
    let mut prefix_done = false;
    let mut unit_line = "".to_string(); //since the defining typeline that tells the meaning of this lines comes after if, it has to be stored until that line is encountered

    for line in text.lines() {
        if line == "[TableValues]" { //table begins, set flag so the next checks are skipped to save cycles.
            prefix_done = true;
            continue;
        } else if !prefix_done {
            if line.contains("Unit=") {//remember this line until the type_line is encountered
                unit_line = line.to_string();
            } else if line.contains("ColTyp=") {
                //line that explains unit_lines meaning encountered, generate output_types now.
                let types_line_parts: Vec<&str> = line.split(['\t', ' ']).collect();
                let mut units_line_parts = unit_line.split(['\t', ' ']).skip(1);
                let mut freq_unit = "".to_string();

                for part in types_line_parts {
                    match part {
                        "Frequency" => {
                            freq_unit = units_line_parts.next().unwrap().to_string();
                        },
                        "Level" => {
                            output_types.push((freq_unit.clone(), units_line_parts.next().unwrap().to_string()))
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
                (line_parts[0].parse::<f64>().unwrap(), line_parts[i].parse::<f64>().unwrap())
            );
        }
    }
    RawData { types: output_types, values: output_values }
}

fn get_limits(file_path: &str) -> Vec<Limit> {
    let mut output: Vec<Limit> = vec![];

    let file_paths: Vec<String> = get_files_with_extension(&(file_path.to_string() + "/System"), "LimitLine").unwrap();

    for (j, file_path) in file_paths.into_iter().enumerate() {
        let text = read_utf16le_file(&file_path).unwrap();
        let mut prefix_done = false;

        //get limit type from filename:
        let name = Path::new(&file_path).file_stem().and_then(|name| name.to_str()).unwrap().to_string();

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
                output[j].values.push((prev_line_parts[0].parse::<f64>().unwrap(), prev_line_parts[1].parse::<f64>().unwrap()));
                output[j].values.push((curr_line_parts[0].parse::<f64>().unwrap(), curr_line_parts[1].parse::<f64>().unwrap()));
            }
            prev_line = curr_line;
        }
    }

    output
}

fn peak_search(data: &[(f64, f64)], limit: &Limit, limit_start_index: usize, limit_end_index: usize) -> Option<Peak>{
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
        if point.1 >= interpolated_limit && point.1 > result.unwrap_or(((-1f64, f64::MIN), -1f64)).0.1{
            //breaks limit and is greater than last break
            result = Some((*point, interpolated_limit));
        }

    }

    result.map(|result| Peak {
            frequency: result.0.0,
            level: result.0.1,
            limit_name: limit.name.clone(),
            limit_level: result.1,
        }) //returns none if no value ever breaks the limit
}

fn build_color_list(colors: &[RGBColor], limits: &[Limit], order: &[&str]) -> Vec<RGBColor> {
    let mut output = vec![];
    for limit in limits {
        for (idx, comp) in order.iter().enumerate() {
            if limit.name.contains(comp) {
                output.push(colors[idx]);
                break;
            }
        }
    }

    output
}

fn plotter(file_path: &Path) -> Result<(), Box<dyn Error>> {

    let file_path = file_path.to_string_lossy();
    let file_path = &file_path.as_ref();

    let conf = CONFIG.get().unwrap();

    //get raw_data and limits from test:
    //it is critical that raw_datas index matches the respective limits index. Sort logic goes through PK/AV/QP shortcut in limit name in /System of test.

    //extract testname from file_path
    let testname = Path::new(file_path).file_name().and_then(|name| name.to_str()).unwrap();

    let raw = get_raw_data(file_path);
    let (types, raw_data) = (raw.types, raw.values);
    let limits: Vec<Limit> = get_limits(file_path);

    if raw_data.is_empty() { //most likely case, Preview Result 1.Result is in binary format, so quit
        println!("| Warning! | Cannot fetch raw data for {}.", file_path);
        println!("             It is most likely still in binary format; Please re-save the test without the EMC32 flag: \"Extras>Options>File/Test>Save EMI Result Tables > 10000 Lines in Binary Format\"");
        return Ok(()); //ok and not error, as this can just be ignored and should not force a stop, like an error would.
    }

    //go through limits and add PK, AV and/or QP to the stack if it exists as a limit at all.

    let mut order = vec![];

    for lim in &limits {
        match &lim.name {
            name if name.contains("PK") => {
                order.push("PK")
            },
            name if name.contains("AV") => {
                order.push("AV")
            },
            name if name.contains("QP") => {
                order.push("QP")
            },
            _ => {},
        }
    }

    order.sort_by_key(|&elem| {
        match elem {
            "PK" => 0,
            "AV" => 1,
            "QP" => 2,
            _ => u8::MAX,
        }
    });


    //plot pre-setup

    let colors_plot = [hex_to_rgb(&conf.graph_color_1)?, hex_to_rgb(&conf.graph_color_2)?, hex_to_rgb(&conf.graph_color_3)?, hex_to_rgb(&conf.graph_color_4)?];
    let colors_limit = [hex_to_rgb(&conf.limit_color_baselevel)?, hex_to_rgb(&conf.limit_color_pk)?, hex_to_rgb(&conf.limit_color_av)?, hex_to_rgb(&conf.limit_color_qp)?];

    //build color order based on naming of limits:
    let color_list_limits: Vec<RGBColor> = build_color_list(&colors_limit, &limits, &["Baselevel", "PK", "AV", "QP"]);

    if raw_data.len() > colors_plot.len() + 1 {panic!()};

    let freqs_plot: Vec<f64> = raw_data[0].iter().map(|(first, _)| first.to_owned()).collect();

    let f_min = freqs_plot.first().unwrap().to_owned();
    let f_max = freqs_plot.last().unwrap().to_owned();

    let (mut y_min, mut y_max) = find_min_max(&raw_data, &limits).unwrap();

    y_min = ((y_min - conf.offset_y_bot)/10.0).round() * 10.0;
    y_max = ((y_max + conf.offset_y_top)/10.0).round() * 10.0;

    let bg_color = hex_to_rgb(&conf.main_color)?;
    let secondary_color = hex_to_rgb(&conf.secondary_color)?;
    let font_style = (conf.style_general_type.as_str(), conf.style_general_size).into_font();
    let font_style_marker = (conf.style_marker_type.as_str(), conf.style_marker_size).into_font();

    if conf.output_dir_name.is_empty(){panic!()};

    fs::create_dir_all(file_path.to_string() + "/" + &conf.output_dir_name)?;

    //for each trace make a graphic:

    for (idx, emi) in raw_data.into_iter().enumerate() {

        generate_raw_data_table(&(file_path.to_string() + &format!("/{}/{}_{}_Values.csv", "!raw_graph", &testname, &order[idx])), &emi, &types[idx])?;

        //begin plotting
        let specific_path = file_path.to_string() + &format!("/{}/{}_{}.svg", conf.output_dir_name, &testname, &order[idx]);
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
        .y_max_light_lines(0)
        .x_max_light_lines(0)
        .label_style(TextStyle::from(font_style.clone()).with_color(secondary_color))
        .draw()?;

        //add crosses
        /*chart.draw_series(emi.iter().map(|&(x, y)| {
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

        // px_left/px_right and py_bottom are already in root pixel space
        let tick_len = conf.custom_ticks_length;
        root.draw(&PathElement::new(
            vec![(px_left, py_bottom), (px_left, py_bottom + tick_len)],
            secondary_color,
        ))?;
        root.draw(&PathElement::new(
            vec![(px_right, py_bottom), (px_right, py_bottom + tick_len)],
            secondary_color,
        ))?;

        //now the tick labels
        let left_style = TextStyle::from(font_style.clone()).pos(Pos::new(HPos::Left, VPos::Top));
        let right_style = TextStyle::from(font_style.clone()).pos(Pos::new(HPos::Right, VPos::Top));

        let gap = conf.custom_ticks_gap;

        root.draw_text(&f_min.to_string(), &left_style, (px_left, py_bottom + tick_len + gap))?;
        root.draw_text(&f_max.to_string(), &right_style, (px_right, py_bottom + tick_len + gap))?;

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
        chart.draw_series(LineSeries::new(emi.clone(), &colors_plot[idx]))?
            .label(order[idx].to_string() + " Measurement Results").legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], colors_plot[idx].stroke_width(conf.legend_line_thickness)));


        let mut peaks: Vec<Peak> = vec![];

        for (lid, lim) in limits.iter().enumerate() { //for each limit
            let limit_name = &lim.name;
            let limit = &lim.values;
            if limit_name.contains(order[idx]) { //check if the limit_name should be plotted
                for i in 0..(limit.len()/2) {
                    if limit[2*i+1].0 <= f_min {continue;} //dont plot sub-limits that end before the graph has even begun
                    if limit[2*i].0 >= f_max {break;} //dont plot sub-limits that start where the graph is already over

                    let peak = peak_search(&emi, lim, 2*i, 2*i+1);
                    //plot the peaks above the limits
                    if let Some(peak) = peak {
                        //push peak to peak table
                        peaks.push(peak);
                    }

                    //plot each limit chunk(breaks if the limit pair format start<->finish is broken):
                    let temp = chart.draw_series(LineSeries::new(vec![limit[2*i], limit[2*i+1]], color_list_limits[lid].stroke_width(3)))?;
                    if i == 0 {
                        //and add it to the legend once
                        let color_clone = color_list_limits[lid];
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

        generate_peak_table(&(file_path.to_string() + &format!("/{}/{}_{}_Limit_Breaks.csv", "!raw_graph", &testname, &order[idx])), peaks, &types[idx])?;

        //write peak values to graph
        //todo!

        let chart_area = chart.plotting_area();
        let (width, height) = chart_area.dim_in_pixel();

        chart_area.draw(&Rectangle::new(
            [(0.0, y_min), (width as f64, height as f64)],
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

fn generate_raw_data_table(file_path: &str, data: &Vec<(f64, f64)>, types: &(String, String)) -> Result<(), Box<dyn Error>> {

    let mut file = File::create(file_path)?;
    file.write_all(&[0xEF, 0xBB, 0xBF])?; //UTF-8 BOM

    let mut wtr = WriterBuilder::new().from_writer(file);

    wtr.serialize("frequency,value")?;
    for datum in data {
        wtr.serialize(format!("{:.2} {},{:.2} {}",datum.0, &types.0, datum.1, &types.1))?;
    }
    wtr.flush()?;
    Ok(())
}

fn generate_peak_table(file_path: &str, peaks: Vec<Peak>, types: &(String, String)) -> Result<(), Box<dyn Error>> {

    let mut file = File::create(file_path)?;
    file.write_all(&[0xEF, 0xBB, 0xBF])?; //UTF-8 BOM

    let mut wtr = WriterBuilder::new().from_writer(file);

    wtr.serialize("limit_name,frequency,value,limit_value")?;
    for peak in &peaks {
        wtr.serialize(format!("{},{:.2} {},{:.2} {},{:.2} {}", peak.limit_name, peak.frequency, types.0, peak.level, types.1, peak.limit_level, types.1))?;
    }
    wtr.flush()?;
    Ok(())
}

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
    Ok(files.iter().map(|v| v.to_str().unwrap().to_owned()).collect())
}

fn find_min_max(data: &[Vec<(f64, f64)>], limits: &[Limit]) -> Option<(f64, f64)> {

    let data: Vec<Vec<f64>> = data.iter().map(|inner|
        inner.iter().map(|(_, val)|
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

//testing to confirm correct functionality. run via "cargo test"

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_01() {
        let testy: Vec<String> = get_files_with_extension("C:\\ProgramData\\EMC32\\Customer Configuration\\SR_Conf_v5_1_beta\\Tests\\tt\\TL81000 (2024) AN L1 9 kHz\\System", "LimitLine").unwrap();
        assert_eq!(testy, vec![
            "C:\\ProgramData\\EMC32\\Customer Configuration\\SR_Conf_v5_1_beta\\Tests\\tt\\TL81000 (2024) AN L1 9 kHz\\System\\TL81000 2024 AN Baseline1 + 9kHz AV Limit Line.LimitLine".to_string(),
            "C:\\ProgramData\\EMC32\\Customer Configuration\\SR_Conf_v5_1_beta\\Tests\\tt\\TL81000 (2024) AN L1 9 kHz\\System\\TL81000 2024 AN Baseline1 + 9kHz PK Limit Line.LimitLine".to_string(),
            "C:\\ProgramData\\EMC32\\Customer Configuration\\SR_Conf_v5_1_beta\\Tests\\tt\\TL81000 (2024) AN L1 9 kHz\\System\\TL81000 2024 AN Class5 9kHz QP Limit Line.LimitLine".to_string()
        ])
    }

    #[test]
    fn test_02() {
        let raw = get_raw_data("C:\\ProgramData\\EMC32\\Customer Configuration\\SR_Conf_v5_1_beta\\Tests\\tt\\TL81000 (2024) AN L1 9 kHz");
        let (types, testy) = (raw.types, raw.values);
        let testy2 = get_limits("C:\\ProgramData\\EMC32\\Customer Configuration\\SR_Conf_v5_1_beta\\Tests\\tt\\TL81000 (2024) AN L1 9 kHz");
        assert_eq!(find_min_max(&testy, &testy2).unwrap(), (3.3058554877, 127.0));
        assert_eq!(types, vec![("MHz".to_string(), "dBμV".to_string()), ("MHz".to_string(), "dBμV".to_string()), ("MHz".to_string(), "dBμV".to_string())])
    }

    #[test]
    fn test_03() {
        let testy = get_limits("C:\\ProgramData\\EMC32\\Customer Configuration\\SR_Conf_v5_1_beta\\Tests\\tt\\TL81000 (2024) AN L1 9 kHz");
        assert_eq!(testy,
            vec![
                Limit{
                    name: "TL81000 2024 AN Baseline1 + 9kHz AV Limit Line".to_string(),
                    values: vec![(0.5200001, 34.0), (1.73, 34.0), (5.0, 65.0), (26.5, 65.0), (26.5000001, 43.0), (29.7, 43.0), (29.7000001, 65.0), (30.0, 65.0), (84.015, 8.0), (87.255, 8.0)],
                },
                Limit{
                    name: "TL81000 2024 AN Baseline1 + 9kHz PK Limit Line".to_string(),
                    values: vec![(0.1, 73.0), (0.15, 73.0), (0.1500001, 127.0), (0.52, 94.87), (0.5200001, 81.0), (5.0, 81.0), (5.0, 75.0), (26.5, 75.0), (26.5000001, 63.0), (29.7, 63.0), (29.7000001, 75.0), (30.0, 75.0), (84.015, 31.0), (87.255, 31.0)],
                },
                Limit{
                    name: "TL81000 2024 AN Class5 9kHz QP Limit Line".to_string(),
                    values: vec![(0.52, 41.0), (1.73, 41.0)],
                },
            ]
        );
    }

    #[test]
    fn test_04() {
        let testy = get_limits("C:\\ProgramData\\EMC32\\Customer Configuration\\SR_Conf_v5_1_beta\\Tests\\tt\\DUT1 MPP Rod Antenna 9k PKCAV");
        assert_eq!(testy,
            vec![
                Limit{
                    name: "VW TL81000 2024 ALSE Baselevel PK Limit".to_string(),
                    values: vec![(0.009, 86.0), (0.15, 62.0), (0.15, 62.0), (0.15, 82.0), (0.15, 82.0), (5.35, 82.0), (5.35, 82.0), (5.35, 62.0), (5.35, 62.0), (20.0, 39.0), (20.0, 39.0), (30.0, 39.0), (30.0, 62.0), (75.0, 52.0), (75.0, 52.0), (400.0, 63.0), (400.0, 63.0), (1000.0, 63.0), (1000.0, 80.0), (3000.0, 80.0), (3000.0, 104.0), (6000.0, 104.0)],
                },
                Limit{
                    name: "VW TL81000 2024 ALSE Class5 9k AV Limit".to_string(),
                    values: vec![(0.52, 18.0), (1.73, 18.0), (26.5, 7.0), (29.7, 7.0)],
                },
                Limit{
                    name: "VW TL81000 2024 ALSE Class5 9k PK Limit".to_string(),
                    values: vec![(0.1, 41.0), (0.15, 41.0), (26.5, 27.0), (29.7, 27.0)],
                },

            ]
        );
    }

    #[test]
    fn test_05() {
        let mut checkee = vec![];
        let order = ["PK", "AV", "QP"];
        let testy = get_raw_data("C:\\ProgramData\\EMC32\\Customer Configuration\\SR_Conf_v5_1_beta\\Tests\\tt\\DUT1 MPP Rod Antenna 9k PKCAV");
        let testy2 = get_limits("C:\\ProgramData\\EMC32\\Customer Configuration\\SR_Conf_v5_1_beta\\Tests\\tt\\DUT1 MPP Rod Antenna 9k PKCAV");
        for (idx, emi) in testy.values.iter().enumerate() {
            for limit in testy2.iter() {
                let lim = &limit.values;
                let name = &limit.name;
                if name.contains(order[idx]) {
                    for i in 0..(lim.len()/2) {
                        let peak = peak_search(emi, limit, 2 * i, 2 * i + 1);
                        if let Some(peak) = peak {
                            checkee.push(peak);
                        }
                    }
                }
            }
        }
        assert_eq!(checkee, vec![ //limit levels are correct despite being so perfect. This occurs due to all peaks happening on flat limits.
            Peak{
                frequency: 26.45425,
                level: 58.724694668,
                limit_name: "VW TL81000 2024 ALSE Baselevel PK Limit".to_string(),
                limit_level: 39.0,
            },
            Peak{
                frequency: 0.12925,
                level: 56.456016197,
                limit_name: "VW TL81000 2024 ALSE Class5 9k PK Limit".to_string(),
                limit_level: 41.0,
            },
            Peak{
                frequency: 26.53975,
                level: 57.392615555,
                limit_name: "VW TL81000 2024 ALSE Class5 9k PK Limit".to_string(),
                limit_level: 27.0,
            },
            Peak{
                frequency: 1.08325,
                level: 36.572282698,
                limit_name: "VW TL81000 2024 ALSE Class5 9k AV Limit".to_string(),
                limit_level: 18.0,
            },
            Peak{
                frequency: 26.53975,
                level: 56.455199478,
                limit_name: "VW TL81000 2024 ALSE Class5 9k AV Limit".to_string(),
                limit_level: 7.0,
            },
        ]);
    }

}
