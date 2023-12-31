use std::{io::Write, path::Path};
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
    pub fn current(&self) -> &str {
        self.files[self.index].path.as_str()
    }

    pub fn current_path(&self) -> &Path {
        Path::new(self.files[self.index].path.as_str())
    }

    fn set_index(&mut self, current: &Path) -> Result<(), ()> {
        let index = self
            .files
            .iter()
            .enumerate()
            .find(|(_, file)| Path::new(&file.path) == current);

        match index {
            Some((index, _)) => {
                self.index = index;
                Ok(())
            }
            None => Err(()),
        }
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

    std::env::set_current_dir("C:\\").unwrap();
    let mut dir = std::env::current_dir().unwrap();
    let mut pages: [Page; 3] = Default::default();

    pages[MIDDLE] = Page {
        files: get_dir(dir.to_str().unwrap()),
        index: 0,
    };

    if let Some(parent) = dir.parent() {
        pages[LEFT] = Page {
            files: get_dir(parent.to_str().unwrap()),
            index: 0,
        };

        //Make sure the previous directory has the current index.
        let _ = pages[LEFT].set_index(parent);
    }

    if let Some(first) = pages[MIDDLE].files.first() {
        pages[RIGHT] = Page {
            files: get_dir(&first.path),
            index: 0,
        };
    }

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
                        pages[RIGHT].files = get_dir(pages[MIDDLE].current());
                        pages[RIGHT].index = 0;
                    }
                    Event::Down if pages[MIDDLE].index + 1 < pages[MIDDLE].files.len() => {
                        pages[MIDDLE].index += 1;
                        pages[RIGHT].files = get_dir(pages[MIDDLE].current());
                        pages[RIGHT].index = 0;
                    }
                    Event::Left => {
                        if let Some(parent) = dir.parent() {
                            dir = parent.to_path_buf();

                            let c = std::mem::take(&mut pages[MIDDLE]);
                            pages[RIGHT] = c;

                            let p = std::mem::take(&mut pages[LEFT]);
                            pages[MIDDLE] = p;

                            if let Some(parent) = dir.parent() {
                                pages[LEFT].files = get_dir(parent.to_str().unwrap());
                                let p = pages[MIDDLE].current_path().to_owned();
                                pages[LEFT].set_index(p.parent().unwrap()).unwrap();
                            }
                        }
                    }
                    Event::Right if pages[MIDDLE].current_path().is_dir() => {
                        let next = get_dir(pages[MIDDLE].current());

                        if !next.is_empty() {
                            dir = pages[MIDDLE].current_path().to_owned();

                            pages[LEFT] = std::mem::take(&mut pages[MIDDLE]);
                            pages[MIDDLE] = std::mem::take(&mut pages[RIGHT]);

                            let right = pages[MIDDLE].current_path();
                            if right.is_dir() {
                                pages[RIGHT].files = get_dir(right.to_str().unwrap());
                                pages[RIGHT].index = 0;
                            } else {
                                pages[RIGHT] = Default::default();
                            }
                        }
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
    let lines: Lines<'_> = text!("{}", pages[MIDDLE].current_path().display())
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
