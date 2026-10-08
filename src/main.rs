use std::{
    error::Error,
    cmp::Ordering,
    path::PathBuf,
    fs::{
        self,
        File
    },
    io::{
        self,
        BufRead,
        BufReader,
        Read
    }
};

use encoding_rs::UTF_16LE;
use encoding_rs_io::DecodeReaderBytesBuilder;

use plotters::prelude::*;
use plotters::style::text_anchor::{Pos, HPos, VPos};

use serde::Serialize;

use rfd::FileDialog;

static PLOT_DIM_X: i32 = 1920; //450 //1920
static PLOT_DIM_Y: i32 = 1080; //340 //1080

static PLOT_COLOR_1: RGBColor = BLUE;
static PLOT_COLOR_2: RGBColor = BLACK;
static PLOT_COLOR_3: RGBColor = MAGENTA;
static PLOT_COLOR_4: RGBColor = GREEN;

static LIMIT_COLOR_PK: RGBColor = GREEN;
static LIMIT_COLOR_AV: RGBColor = RED;
static LIMIT_COLOR_QP: RGBColor = GREEN;
static LIMIT_COLOR_BASELEVEL: RGBColor = BLACK;

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
    plotter("C:\\ProgramData\\EMC32\\Customer Configuration\\SR_Conf_v5_1_beta\\Tests\\tt\\TL81000 (2024) AN L1 9 kHz").unwrap();
    plotter("C:\\ProgramData\\EMC32\\Customer Configuration\\SR_Conf_v5_1_beta\\Tests\\tt\\DUT1 MPP Rod Antenna 9k PKCAV").unwrap();
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

fn get_raw_data(file_path: &str) -> RawData { //(Vec<(Freq Unit: String, Level Unit: String)>, Vec<Vec<(Freq: f64, Level: f64)>>)
    let mut output_values = vec![];
    let mut output_types = vec![];
    let text = read_utf16le_file(&(file_path.to_string() + "/Preview Result 1.Result")).unwrap();
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
            }
            continue;
        }
        //value table handling:

        //read line to output
        let line = line.split(['L', 'N', 'V', 'H']).collect::<Vec<_>>().first().unwrap().to_owned(); //cuts off when L(1) or N is encountered in the table
        let line_parts: Vec<&str> = line.split(['\t', ' ']).collect();

        //pre-create vectors
        if output_values.is_empty() {
            output_values = vec![Vec::new(); line_parts.len()-2]; //-2 since no extra place for frequency column(-1) and the last element is always empty(-1)
        }

        for i in 1..line_parts.len() - 1 { //-1 since the last element is always empty
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
        let name = std::path::Path::new(&file_path).file_stem().and_then(|name| name.to_str()).unwrap().to_string();

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

fn plotter(file_path: &str) -> Result<(), Box<dyn Error>> {

    let order = ["PK", "AV", "QP"];

    //get raw_data and limits from test:
    //it is critical that raw_datas index matches the respective limits index. Sort logic goes through PK/AV/QP shortcut in limit name in /System of test.

    //extract testname from file_path
    let testname = std::path::Path::new(file_path).file_name().and_then(|name| name.to_str()).unwrap();

    let raw = get_raw_data(file_path);
    let (types, raw_data) = (raw.types, raw.values);
    let limits: Vec<Limit> = get_limits(file_path);

    //plot pre-setup

    let colors_plot = [PLOT_COLOR_1, PLOT_COLOR_2, PLOT_COLOR_3, PLOT_COLOR_4];
    let colors_limit = [LIMIT_COLOR_BASELEVEL, LIMIT_COLOR_PK, LIMIT_COLOR_AV, LIMIT_COLOR_QP];

    //build color order based on naming of limits:
    let color_list_limits: Vec<RGBColor> = build_color_list(&colors_limit, &limits, &["Baselevel", "PK", "AV", "QP"]);

    if raw_data.len() - 1 > colors_plot.len(){panic!()};

    let freqs_plot: Vec<f64> = raw_data[0].iter().map(|(first, _)| first.to_owned()).collect();

    let f_min = freqs_plot.first().unwrap().to_owned();
    let f_max = freqs_plot.last().unwrap().to_owned();

    let (mut y_min, mut y_max) = find_min_max(&raw_data, &limits).unwrap();

    y_min = ((y_min - 20.0)/10.0).round() * 10.0;
    y_max = ((y_max + 10.0)/10.0).round() * 10.0;

    let bg_color = WHITE;
    let secondary_color = BLACK;
    let font_style = ("sans-serif", 30).into_font();

    fs::create_dir_all(file_path.to_string() + "/!raw_graph")?;

    //for each trace make a graphic:

    for (idx, emi) in raw_data.into_iter().enumerate() {

        //begin plotting
        let specific_path = file_path.to_string() + &format!("/{}/{}_{}.svg", "!raw_graph", &testname, &order[idx]);
        let root = SVGBackend::new(&specific_path, (PLOT_DIM_X.try_into()?, PLOT_DIM_Y.try_into()?)).into_drawing_area();
        root.fill(&bg_color)?;

        let mut chart = ChartBuilder::on(&root)
            .caption(format!("{} {}", testname, &order[idx]), ("sans-serif", 30).into_font().color(&secondary_color))
            .x_label_area_size(75)
            .y_label_area_size(100)
            .margin(50)
            .build_cartesian_2d((f_min .. f_max).log_scale(), y_min..y_max)?;

        chart.configure_mesh()
        .x_desc(format!("Frequency / {}", types[idx].0))
        .y_desc(format!("Emissions / {}", types[idx].1))
        .axis_desc_style(font_style.clone().with_color(secondary_color))
        .bold_line_style(secondary_color.mix(0.3).stroke_width(1))
        .light_line_style(secondary_color.mix(0.3).stroke_width(1))
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
                    BLACK.mix(0.3)
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
                    |c| Circle::new(c, 1, BLACK.mix(0.3).filled()),
                )
            )?;
        }

        //and now the labels:

        let plotting_area = chart.plotting_area();

        let (px_left, py_bottom) = plotting_area.map_coordinate(&(f_min, y_min));
        let (px_right, _)        = plotting_area.map_coordinate(&(f_max, y_min));

        // px_left/px_right and py_bottom are already in root pixel space
        let tick_len = 30i32;
        root.draw(&PathElement::new(
            vec![(px_left, py_bottom), (px_left, py_bottom + tick_len)],
            BLACK,
        ))?;
        root.draw(&PathElement::new(
            vec![(px_right, py_bottom), (px_right, py_bottom + tick_len)],
            BLACK,
        ))?;

        //now the tick labels
        let style = TextStyle::from(font_style.clone()).pos(Pos::new(HPos::Center, VPos::Top));

        let gap = 4i32;

        root.draw_text(&f_min.to_string(), &style, (px_left, py_bottom + tick_len + gap))?;
        root.draw_text(&f_max.to_string(), &style, (px_right, py_bottom + tick_len + gap))?;

        let mut y = y_min;
        while y < y_max {
            y += 10.0;
            if y % 20.0 > 0.3 {
                chart.draw_series(DottedLineSeries::new(
                    vec![(f_min, y), (f_max, y)],
                    0,
                    10,
                    |c| Circle::new(c, 1, BLACK.mix(0.3).filled()),
                ))?;
            }
        }

        //chart actual data the cloning of the data could be avoided if the peak_search was adjusted to run once at the beginning
        //that would double the looping over limits. Its speed vs space and this configuration focuses more on speed.
        chart.draw_series(LineSeries::new(emi.clone(), &colors_plot[idx]))?
            .label(order[idx].to_string() + " Measurement Results").legend(|(x, y)| PathElement::new(vec![(x, y), (x + 20, y)], colors_plot[idx].stroke_width(3)));



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

                        //plot the peak as a triangle

                        let (px, py) = chart.plotting_area().map_coordinate(&(peak.frequency, peak.level));
                        let half_width: i32 = 8;
                        let height: i32 = 16;

                        let vertices = vec![
                            (px, py),                       // tip (bottom)
                            (px - half_width, py - height), // top-left
                            (px + half_width, py - height), // top-right
                            (px, py),                       // close path
                        ];

                        let triangle = Polygon::new(vertices, BLACK);
                        root.draw(&triangle)?;

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

        generate_table(&(file_path.to_string() + &format!("/{}/{}_{}.csv", "!raw_graph", &testname, &order[idx])), peaks)?;

        let chart_area = chart.plotting_area();
        let (width, height) = chart_area.dim_in_pixel();

        chart_area.draw(&Rectangle::new(
            [(0.0, y_min), (width as f64, height as f64)],
            ShapeStyle::from(&BLACK).stroke_width(2)
        ))?;

        //position legend correctly
        chart.configure_series_labels()
            .position(SeriesLabelPosition::LowerRight)
            .label_font(font_style.clone())
            .border_style(BLACK)
            .background_style(WHITE)
            .draw()?;




        root.present()?;

    }



    Ok(())
}

fn generate_table(file_path: &str, peaks: Vec<Peak>) -> Result<(), Box<dyn Error>> {
    let mut wtr = csv::Writer::from_path(file_path)?;
    for peak in &peaks {
        wtr.serialize(peak)?;
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
