# README example

`glance-example.png` is a real Glance export using the built-in practice canvas,
a spotlight, a magnifier, and the Lava backdrop at phase 0.2. It contains no
personal screen content.

To regenerate it after building the native helpers:

```sh
cargo test --release --locked focus_and_loop_demo_qa -- --ignored --nocapture
cp target/focus-qa/focus-demo.png docs/assets/glance-example.png
```

That opt-in test also produces a GIF and MP4 in `target/focus-qa/`; only the PNG
is tracked here. The image uses the same compositor as the app's exports.
