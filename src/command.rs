fn parse_command(command: &str) -> Box<dyn Action> {
    match command {
        ":q" => Quit::new();

    }
}
