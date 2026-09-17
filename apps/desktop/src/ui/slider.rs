use gpui::{
    App, Bounds, DispatchPhase, Element, ElementId, GlobalElementId, Hsla, InspectorElementId,
    IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, Size, Style, Window, fill, hsla, px, relative,
};
use std::cell::RefCell;
use std::ops::RangeInclusive;
use std::rc::Rc;
use std::time::{Duration, Instant};

const DRAG_COMMIT_INTERVAL: Duration = Duration::from_millis(120);
const DRAG_PAINT_INTERVAL: Duration = Duration::from_millis(33);

struct ActiveSliderDrag {
    id: ElementId,
    value: f32,
    last_commit: Instant,
    last_paint: Instant,
}

thread_local! {
    static ACTIVE_SLIDER_DRAG: RefCell<Option<ActiveSliderDrag>> = const { RefCell::new(None) };
}

pub fn slider(id: impl Into<ElementId>, value: f32) -> Slider {
    Slider::new(id, value)
}

pub struct Slider {
    id: ElementId,
    value: f32,
    min: f32,
    max: f32,
    step: Option<f32>,
    disabled: bool,
    active_color: Hsla,
    inactive_color: Hsla,
    thumb_color: Hsla,
    on_change: Option<Rc<dyn Fn(f32, &mut App)>>,
}

// The builder exposes the full reusable slider API; not every setting uses
// every configuration method.
#[allow(dead_code)]
impl Slider {
    pub fn new(id: impl Into<ElementId>, value: f32) -> Self {
        Self {
            id: id.into(),
            value,
            min: 0.0,
            max: 1.0,
            step: None,
            disabled: false,
            active_color: hsla(0.58, 0.8, 0.6, 1.0),
            inactive_color: hsla(0.0, 0.0, 1.0, 0.12),
            thumb_color: hsla(0.0, 0.0, 1.0, 0.95),
            on_change: None,
        }
    }

    pub fn min(mut self, min: f32) -> Self {
        self.min = min;
        self
    }

    pub fn max(mut self, max: f32) -> Self {
        self.max = max;
        self
    }

    pub fn range(mut self, range: RangeInclusive<f32>) -> Self {
        self.min = *range.start();
        self.max = *range.end();
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        self.step = (step.is_finite() && step > 0.0).then_some(step);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn colors(mut self, active: Hsla, inactive: Hsla, thumb: Hsla) -> Self {
        self.active_color = active;
        self.inactive_color = inactive;
        self.thumb_color = thumb;
        self
    }

    pub fn on_change(mut self, on_change: impl Fn(f32, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(on_change));
        self
    }
}

impl IntoElement for Slider {
    type Element = SliderElement;

    fn into_element(self) -> Self::Element {
        SliderElement {
            id: self.id,
            value: self.value,
            min: self.min,
            max: self.max,
            step: self.step,
            disabled: self.disabled,
            active_color: self.active_color,
            inactive_color: self.inactive_color,
            thumb_color: self.thumb_color,
            on_change: self.on_change,
        }
    }
}

pub struct SliderElement {
    id: ElementId,
    value: f32,
    min: f32,
    max: f32,
    step: Option<f32>,
    disabled: bool,
    active_color: Hsla,
    inactive_color: Hsla,
    thumb_color: Hsla,
    on_change: Option<Rc<dyn Fn(f32, &mut App)>>,
}

impl IntoElement for SliderElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for SliderElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        // Keep a generous hit target around the visual track. This makes the
        // control usable without requiring pixel-precise pointer placement.
        style.size.height = px(32.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        let min = self.min;
        let max = self.max;
        let step = self.step;
        let disabled = self.disabled;
        if !disabled {
            if let Some(on_change) = self.on_change.clone() {
                let compute_value = move |pos_x: Pixels| -> f32 {
                    let ratio = if bounds.size.width > px(0.0) {
                        ((pos_x - bounds.origin.x) / bounds.size.width).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let mut value = min + ratio * (max - min);
                    if let Some(step) = step {
                        value = (value / step).round() * step;
                    }
                    value.clamp(min, max)
                };

                let slider_id_on_down = self.id.clone();
                let compute_on_down = compute_value.clone();
                let on_change_on_down = on_change.clone();
                window.on_mouse_event(move |event: &MouseDownEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble
                        && event.button == MouseButton::Left
                        && bounds.contains(&event.position)
                    {
                        let now = Instant::now();
                        let value = compute_on_down(event.position.x);
                        ACTIVE_SLIDER_DRAG.with(|active| {
                            *active.borrow_mut() = Some(ActiveSliderDrag {
                                id: slider_id_on_down.clone(),
                                value,
                                last_commit: now,
                                last_paint: now,
                            });
                        });
                        on_change_on_down(value, cx);
                        cx.stop_propagation();
                    }
                });

                let slider_id_on_move = self.id.clone();
                let on_change_on_move = on_change.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase != DispatchPhase::Bubble {
                        return;
                    }
                    let now = Instant::now();
                    let value = compute_value(event.position.x);
                    let (should_commit, should_refresh) = ACTIVE_SLIDER_DRAG.with(|active| {
                        let mut active = active.borrow_mut();
                        let Some(drag) = active.as_mut() else {
                            return (false, false);
                        };
                        if drag.id != slider_id_on_move {
                            return (false, false);
                        }
                        drag.value = value;
                        let should_commit =
                            now.duration_since(drag.last_commit) >= DRAG_COMMIT_INTERVAL;
                        let should_refresh =
                            now.duration_since(drag.last_paint) >= DRAG_PAINT_INTERVAL;
                        if should_commit {
                            drag.last_commit = now;
                        }
                        if should_refresh {
                            drag.last_paint = now;
                        }
                        (should_commit, should_refresh)
                    });
                    if should_commit {
                        on_change_on_move(value, cx);
                    }
                    if should_refresh {
                        window.refresh();
                    }
                });

                let slider_id_on_up = self.id.clone();
                let compute_on_up = compute_value.clone();
                let on_change_on_up = on_change.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                    if phase != DispatchPhase::Bubble {
                        return;
                    }
                    let was_dragging = ACTIVE_SLIDER_DRAG.with(|active| {
                        active
                            .borrow()
                            .as_ref()
                            .is_some_and(|drag| drag.id == slider_id_on_up)
                    });
                    if was_dragging {
                        ACTIVE_SLIDER_DRAG.with(|active| *active.borrow_mut() = None);
                        // Always commit the final pointer position so throttling
                        // never leaves the persisted value behind the thumb.
                        on_change_on_up(compute_on_up(event.position.x), cx);
                    }
                });
            }
        }

        let display_value = ACTIVE_SLIDER_DRAG.with(|active| {
            active
                .borrow()
                .as_ref()
                .filter(|drag| drag.id == self.id)
                .map_or(self.value, |drag| drag.value)
        });
        let track_height = px(6.0);
        let track_bounds = Bounds {
            origin: Point {
                x: bounds.origin.x,
                y: bounds.origin.y + (bounds.size.height - track_height) / 2.0,
            },
            size: Size {
                width: bounds.size.width,
                height: track_height,
            },
        };

        let track_bg = self.inactive_color;
        let active_bg = if disabled {
            self.inactive_color
        } else {
            self.active_color
        };
        let thumb_color = self.thumb_color;

        window.paint_quad(fill(track_bounds, track_bg).corner_radii(px(2.0)));

        let ratio = if max > min && display_value.is_finite() {
            ((display_value - min) / (max - min)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let active_bounds = Bounds {
            origin: track_bounds.origin,
            size: Size {
                width: bounds.size.width * ratio,
                height: track_height,
            },
        };
        window.paint_quad(fill(active_bounds, active_bg).corner_radii(px(2.0)));

        let thumb_radius = px(9.0);
        let thumb_center_x = bounds.origin.x + bounds.size.width * ratio;
        let thumb_bounds = Bounds {
            origin: Point {
                x: thumb_center_x - thumb_radius,
                y: bounds.origin.y + (bounds.size.height - thumb_radius * 2.0) / 2.0,
            },
            size: Size {
                width: thumb_radius * 2.0,
                height: thumb_radius * 2.0,
            },
        };
        window.paint_quad(fill(thumb_bounds, thumb_color).corner_radii(thumb_radius));
    }
}
