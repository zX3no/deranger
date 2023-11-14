use std::{io::Write, path::PathBuf};
use winter::*;
use winwalk::*;

const LEFT: usize = 0;
const MIDDLE: usize = 1;
const RIGHT: usize = 2;

#[derive(Debug, Default)]
pub struct Page {
    pub files: Vec<DirEntry>,
    pub index: usize,
}

impl Page {
    #[track_caller]
    pub fn current(&self) -> PathBuf {
        self.files[self.index].path.clone()
    }

    fn set_index(&mut self, current: PathBuf) {
        self.index = self
            .files
            .iter()
            .enumerate()
            .find(|(_, file)| file.path == current)
            .unwrap()
            .0;
    }
}

fn get_dir(dir: &str) -> Vec<DirEntry> {
    walkdir(dir, 1).into_iter().flatten().collect()
}

fn main() {
    let (output_handle, _) = handles();
    let (width, height) = info(output_handle).window_size;

    let mut viewport = Rect::new(0, 0, width, height);
    let mut buffers: [Buffer; 2] = [Buffer::empty(viewport), Buffer::empty(viewport)];
    let mut current = 0;

    //Prevents panic messages from being hidden.
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let mut stdout = std::io::stdout();
        uninit(&mut stdout);
        stdout.flush().unwrap();
        orig_hook(panic_info);
        std::process::exit(1);
    }));

    //TODO: Might need to wrap stdout, viewport and current buffer.
    //v.area(), v.stdout(), v.buffer(). maybe maybe not.
    let mut stdout = std::io::stdout();
    init(&mut stdout);

    let mut dir = std::env::current_dir().unwrap();
    let mut pages: [Page; 3] = Default::default();

    pages[MIDDLE] = Page {
        files: get_dir(dir.to_str().unwrap()),
        index: 0,
    };

    let prev_path = dir.parent().unwrap().to_str().unwrap();
    pages[LEFT] = Page {
        files: get_dir(prev_path),
        index: 0,
    };

    //Make sure the previous directory has the current index.
    pages[LEFT].set_index(pages[MIDDLE].current().parent().unwrap().to_path_buf());

    pages[RIGHT] = Page {
        files: get_dir(pages[MIDDLE].files[0].path.to_str().unwrap()),
        index: 0,
    };

    loop {
        //Draw the widgets into the front buffer.
        draw(viewport, &mut buffers[current], pages.as_slice());

        //Handle events
        {
            if let Some((event, state)) = poll(std::time::Duration::from_millis(16)) {
                match event {
                    Event::Up => {
                        if pages[MIDDLE].index != 0 {
                            pages[MIDDLE].index -= 1;
                        }
                        pages[RIGHT].files = get_dir(pages[MIDDLE].current().to_str().unwrap());
                        pages[RIGHT].index = 0;
                    }
                    Event::Down if pages[MIDDLE].index + 1 < pages[MIDDLE].files.len() => {
                        pages[MIDDLE].index += 1;
                        pages[RIGHT].files = get_dir(pages[MIDDLE].current().to_str().unwrap());
                        pages[RIGHT].index = 0;
                    }
                    Event::Left => {
                        dir = dir.parent().unwrap().to_path_buf();

                        let c = std::mem::take(&mut pages[MIDDLE]);
                        pages[RIGHT] = c;

                        let p = std::mem::take(&mut pages[LEFT]);
                        pages[MIDDLE] = p;

                        if let Some(parent) = dir.parent() {
                            pages[LEFT].files = get_dir(parent.to_str().unwrap());
                            pages[LEFT]
                                .set_index(pages[MIDDLE].current().parent().unwrap().to_path_buf());
                        }
                    }
                    Event::Right => {
                        // A -> B -> C
                        // B -> C -> D
                        // A(LEFT) is deleted. D(RIGHT) is created.
                        // C(Right) has the wrong index!

                        dir = pages[MIDDLE].current();

                        let middle = std::mem::take(&mut pages[MIDDLE]);
                        pages[LEFT] = middle;

                        let right = std::mem::take(&mut pages[RIGHT]);
                        pages[MIDDLE] = right;

                        pages[RIGHT].files = get_dir(pages[MIDDLE].current().to_str().unwrap());
                    }
                    Event::Char('c') if state.ctrl() => break,
                    Event::Escape => break,
                    _ => {}
                }
            }
        }

        //Calculate difference and draw to the terminal.
        let previous_buffer = &buffers[1 - current];
        let current_buffer = &buffers[current];
        let diff = previous_buffer.diff(current_buffer);
        buffer::draw(&mut stdout, diff);

        //Swap buffers
        buffers[1 - current].reset();
        current = 1 - current;

        //Update the viewport area.
        //TODO: I think there is a resize event that might be better.
        let (width, height) = info(output_handle).window_size;
        viewport = Rect::new(0, 0, width, height);

        //Resize
        if buffers[current].area != viewport {
            buffers[current].resize(viewport);
            buffers[1 - current].resize(viewport);

            // Reset the back buffer to make sure the next update will redraw everything.
            buffers[1 - current].reset();
            clear(&mut stdout);
        }

        // break;
    }

    uninit(&mut stdout);
}

fn draw(area: Rect, buffer: &mut Buffer, pages: &[Page]) {
    let h = layout!(
        area,
        Direction::Vertical,
        Constraint::Length(3),
        Constraint::Min(100)
    );
    let layout = layout!(
        h[1],
        Direction::Horizontal,
        Constraint::Percentage(15),
        Constraint::Percentage(45),
        Constraint::Percentage(30)
    );

    //Draw the current path
    let lines: Lines<'_> = text!("{}", pages[MIDDLE].current().display())
        .into_lines()
        .block(None, Borders::ALL, Rounded);
    lines.draw(h[0], buffer);

    for (i, area) in layout.iter().enumerate() {
        let text: Vec<Lines<'_>> = pages[i]
            .files
            .iter()
            .map(|path| text!("{}", path.name).into())
            .collect();
        let list = list(
            Some(block(None, Borders::ALL, BorderType::Rounded)),
            text,
            Some("> "), //TODO: Would using "" as an empty selector be better than None?
            Some(bg(Blue)),
        );

        list.draw(
            *area,
            buffer,
            if i == MIDDLE {
                Some(pages[i].index)
            } else {
                None
            },
        );
    }
}
