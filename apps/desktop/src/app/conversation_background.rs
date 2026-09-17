use super::*;

/// Conversation backgrounds are decorative, so they are capped well below the
/// 48 MB wire limit to keep daemon uploads and transcript frames cheap.
pub(super) const MAX_BACKGROUND_IMAGE_BYTES: usize = 2 * 1024 * 1024;

/// Loads and paints the conversation background without doing filesystem work
/// from the transcript frame path.
impl Padu {
    pub(super) fn set_conversation_background_opacity(
        &mut self,
        value: f32,
        cx: &mut Context<Self>,
    ) {
        self.state.conversation_background.opacity = value.clamp(0.0, 1.0);
        self.save();
        cx.notify();
    }

    pub(super) fn set_conversation_background_height(
        &mut self,
        value: f32,
        cx: &mut Context<Self>,
    ) {
        self.state.conversation_background.height_percent = value.clamp(20.0, 100.0);
        self.save();
        cx.notify();
    }

    pub(super) fn conversation_background_image(&self) -> Option<Arc<gpui::Image>> {
        self.conversation_background_image.borrow().clone()
    }

    pub(super) fn load_persisted_conversation_background(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.state.conversation_background.image_path.clone() else {
            return;
        };
        let Some(format) =
            crate::app::image_preview::image_format_for_name(path.to_string_lossy().as_ref())
        else {
            return;
        };
        let generation = self
            .conversation_background_generation
            .get()
            .wrapping_add(1);
        self.conversation_background_generation.set(generation);
        cx.spawn(async move |this, cx| {
            let loaded = cx
                .background_executor()
                .spawn(async move { std::fs::read(&path).map(|bytes| (format, bytes)) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.conversation_background_generation.get() != generation {
                    return;
                }
                if let Ok((format, bytes)) = loaded {
                    if !bytes.is_empty() {
                        this.conversation_background_image
                            .borrow_mut()
                            .replace(Arc::new(gpui::Image::from_bytes(format, bytes)));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(super) fn choose_conversation_background(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("settings.background_choose").into()),
        });
        let generation = self
            .conversation_background_generation
            .get()
            .wrapping_add(1);
        self.conversation_background_generation.set(generation);
        cx.spawn(async move |this, cx| {
            let result = receiver
                .await
                .ok()
                .and_then(|result| result.ok())
                .and_then(|paths| paths.and_then(|paths| paths.into_iter().next()));
            let Some(path) = result else { return };
            let Some(format) =
                crate::app::image_preview::image_format_for_name(path.to_string_lossy().as_ref())
            else {
                let _ = this.update(cx, |this, cx| {
                    this.show_toast(tr!("settings.background_unsupported"));
                    cx.notify();
                });
                return;
            };
            let loaded = cx
                .background_executor()
                .spawn(async move { std::fs::read(&path).map(|bytes| (path, format, bytes)) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.conversation_background_generation.get() != generation {
                    return;
                }
                match loaded {
                    Ok((path, format, bytes))
                        if !bytes.is_empty() && bytes.len() <= MAX_BACKGROUND_IMAGE_BYTES =>
                    {
                        this.state.conversation_background.image_path = Some(path);
                        this.conversation_background_image
                            .borrow_mut()
                            .replace(Arc::new(gpui::Image::from_bytes(format, bytes)));
                        this.save();
                    }
                    Ok((_, _, bytes)) if bytes.len() > MAX_BACKGROUND_IMAGE_BYTES => {
                        this.show_toast(tr!("settings.background_too_large"));
                    }
                    _ => this.show_toast(tr!("settings.background_unavailable")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn clear_conversation_background(&mut self, cx: &mut Context<Self>) {
        self.conversation_background_generation.set(
            self.conversation_background_generation
                .get()
                .wrapping_add(1),
        );
        self.state.conversation_background.image_path = None;
        self.conversation_background_image.borrow_mut().take();
        self.save();
        cx.notify();
    }
}

pub(super) fn render_conversation_background(
    image: Option<Arc<gpui::Image>>,
    settings: &padu_client::persistence::ConversationBackgroundSettings,
    theme: &Theme,
) -> Option<AnyElement> {
    let image = image?;
    Some(
        div()
            .absolute()
            .left_0()
            .right_0()
            .top_0()
            .h(gpui::relative(settings.height_percent / 100.0))
            .overflow_hidden()
            .child(
                img(image)
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .opacity(settings.opacity)
                    .flex_none(),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(gpui::relative(0.55))
                    .bg(linear_gradient(
                        180.0,
                        linear_color_stop(theme.canvas.opacity(0.0), 0.0),
                        linear_color_stop(theme.canvas, 1.0),
                    )),
            )
            .into_any_element(),
    )
}
