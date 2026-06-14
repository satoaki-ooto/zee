use palette::convert::FromColorUnclamped;
use palette::{Hsv, Srgb, Mix};
use std::borrow::Cow;
use zi::{
    components::text::{Text, TextProperties},
    prelude::*,
};

#[derive(Clone, PartialEq)]
pub struct StatusProperties {
    pub action_name: Cow<'static, str>,
    pub pending: bool,
    pub style: Style,
}

pub struct Status {
    properties: StatusProperties,
    animation_offset: f32,
    endpoints: [Hsv; 2],
}

impl Component for Status {
    type Message = ();
    type Properties = StatusProperties;

    fn create(properties: Self::Properties, _frame: Rect, _link: ComponentLink<Self>) -> Self {
        Self {
            endpoints: endpoints_from_style(properties.style),
            properties,
            animation_offset: 1.0,
        }
    }

    fn change(&mut self, properties: Self::Properties) -> ShouldRender {
        if self.properties != properties {
            self.endpoints = endpoints_from_style(properties.style);
            if self.properties.pending != properties.pending {
                self.animation_offset = 1.0;
            }
            self.properties = properties;
            ShouldRender::Yes
        } else {
            ShouldRender::No
        }
    }

    fn update(&mut self, _message: Self::Message) -> ShouldRender {
        // `animation_offset` ticks in the interval [0, 2]:
        self.animation_offset = (self.animation_offset + 0.125) % 2.0;
        ShouldRender::Yes
    }

    fn view(&self) -> Layout {
        let Self {
            properties:
                StatusProperties {
                    ref action_name,
                    style,
                    pending,
                },
            ..
        } = *self;

        let style = if pending {
            self.animated_style()
        } else {
            style
        };
        Text::with(
            TextProperties::new()
                .content(action_name.to_owned())
                .style(style),
        )
    }

    fn tick(&self) -> Option<Self::Message> {
        if self.properties.pending {
            Some(())
        } else {
            None
        }
    }
}

fn endpoints_from_style(style: Style) -> [Hsv; 2] {
    [
        Hsv::from_color_unclamped(
            Srgb::new(
                style.background.red,
                style.background.green,
                style.background.blue,
            )
            .into_format::<f32>(),
        ),
        Hsv::from_color_unclamped(
            Srgb::new(
                style.foreground.red,
                style.foreground.green,
                style.foreground.blue,
            )
            .into_format::<f32>(),
        ),
    ]
}

impl Status {
    fn animated_style(&self) -> Style {
        let t = (self.animation_offset - 1.0).abs();
        let background =
            Srgb::from_color_unclamped(self.endpoints[0].mix(self.endpoints[1], t))
                .into_format::<u8>();
        let foreground =
            Srgb::from_color_unclamped(self.endpoints[0].mix(self.endpoints[1], 1.0 - t))
                .into_format::<u8>();

        Style::normal(
            Colour {
                red: background.red,
                green: background.green,
                blue: background.blue,
            },
            Colour {
                red: foreground.red,
                green: foreground.green,
                blue: foreground.blue,
            },
        )
    }
}

// const PROGRESS_PATTERN: [char; 16] = [
//     '⣾', '⣽', '⣻', '⢿', '⡿', '⣟', '⣯', '⣷', '⠁', '⠂', '⠄', '⡀', '⢀', '⠠', '⠐', '⠈',
// ];
// const PROGRESS_PATTERN: [char; 13] = [
//     '▉', '▊', '▋', '▌', '▍', '▎', '▏', '▎', '▍', '▌', '▋', '▊', '▉',
// ];
// const PROGRESS_PATTERN: [char; 8] = ['▙', '▛', '▜', '▟', '▘', '▝', '▖', '▗'];
// const PROGRESS_PATTERN: [char; 6] = ['◜', '◠', '◝', '◞', '◡', '◟'];
// const PROGRESS_PATTERN: [char; 4] = ['■', '□', '▪', '▫'];
// const PROGRESS_PATTERN: [char; 8] = ['▘', '▀', '▝', '▐', '▗', '▄', '▖', '▌'];
// const PROGRESS_PATTERN: [char; 29] = [
//     '⠁', '⠁', '⠉', '⠙', '⠚', '⠒', '⠂', '⠂', '⠒', '⠲', '⠴', '⠤', '⠄', '⠄', '⠤', '⠠', '⠠', '⠤', '⠦',
//     '⠖', '⠒', '⠐', '⠐', '⠒', '⠓', '⠋', '⠉', '⠈', '⠈',
// ];
