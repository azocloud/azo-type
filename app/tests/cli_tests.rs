use std::path::PathBuf;
use azo_type::parse_cli_args_from;

#[test]
fn parses_empty_cli_arguments_as_none() {
    let args = vec!["typst-ide".to_string()];
    let (folder, screenshot) = parse_cli_args_from(&args);
    assert_eq!(folder, None);
    assert_eq!(screenshot, None);
}

#[test]
fn parses_screenshot_flag_with_space_and_equals() {
    let args_space = vec![
        "typst-ide".to_string(),
        "--screenshot".to_string(),
        "out/shot.png".to_string(),
    ];
    let (_, shot_space) = parse_cli_args_from(&args_space);
    assert_eq!(shot_space, Some(PathBuf::from("out/shot.png")));

    let args_eq = vec![
        "typst-ide".to_string(),
        "--screenshot=target/capture.png".to_string(),
    ];
    let (_, shot_eq) = parse_cli_args_from(&args_eq);
    assert_eq!(shot_eq, Some(PathBuf::from("target/capture.png")));
}

#[test]
fn parses_folder_path_when_directory_exists() {
    let temp_dir = std::env::temp_dir();
    let args = vec![
        "typst-ide".to_string(),
        temp_dir.to_string_lossy().to_string(),
    ];
    let (folder, _) = parse_cli_args_from(&args);
    assert_eq!(folder, Some(temp_dir));
}

#[test]
fn ignores_nonexistent_directory_arguments() {
    let args = vec![
        "typst-ide".to_string(),
        "/nonexistent/path/for/cli/test/98765".to_string(),
    ];
    let (folder, _) = parse_cli_args_from(&args);
    assert_eq!(folder, None);
}
