use iced::{button, Application, Button, Column, Command, Element, Settings, Text};

mod replicant;
use replicant::Semilattice;


#[derive(Default)]
struct Counter {
    // The counter value
    value: i32,

    // The local state of the two buttons
    increment_button: button::State,
    decrement_button: button::State,
}


#[derive(Debug, Clone, Copy)]
pub enum Message {
    IncrementPressed,
    DecrementPressed,
}


impl Application for Counter {
    type Message = Message;
    
    fn new() -> (Self, Command<Message>) {
        (Self::default(), Command::none())
    }

    
    fn title(&self) -> String {
        String::from("A simple counter")
    }

    
    fn view(&mut self) -> Element<Message> {
        Column::new()
            .push(
                Button::new(&mut self.increment_button, Text::new("Increment"))
                    .on_press(Message::IncrementPressed),
            )
            .push(
                Text::new(self.value.to_string()).size(50),
            )
            .push(
                Button::new(&mut self.decrement_button, Text::new("Decrement"))
                    .on_press(Message::DecrementPressed),
            )
            .into()
    }

    
    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::IncrementPressed => {
                self.value += 1;
            }
            Message::DecrementPressed => {
                self.value = if self.value > 0 { self.value - 1} else { self.value };
            }
        }

        Command::none()
    }
}


fn main() {
    Counter::run(Settings::default());
    let nat = replicant::Nat { value: 3 };
    println!("{}", replicant::Nat::NAME);
}
