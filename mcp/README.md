# Pachiri local MCP companion

ChatGPT or another MCP client can edit the **real native GPUI window** using structured tools. GPUI stays native; there is no web canvas or screenshot-click automation. The stdio companion connects to the opted-in editor over a private Unix socket.

## Build and start

```sh
./scripts/bundle.sh release
./target/Pachiri.app/Contents/MacOS/Pachiri --automation
```

Start this build as your editor. If another automation-enabled Pachiri is already running, stop that instance first. Ordinary launches without `--automation` do not expose the bridge. Keep the editor running while using MCP.

Configure a local stdio MCP client with an **absolute path** to this checkout's `scripts/mcp.sh`:

```json
{
  "mcpServers": {
    "pachiri": {
      "command": "/absolute/path/to/pachiri/scripts/mcp.sh",
      "args": []
    }
  }
}
```

For an installed bundle you can instead use `/Applications/Pachiri.app/Contents/MacOS/Pachiri` as the command with `args: ["--mcp"]`, provided that bundle contains this build. Launch that same bundle's executable with `--automation` for the native editor.

## Connect ChatGPT

Use [Secure MCP Tunnel](https://developers.openai.com/api/docs/guides/secure-mcp-tunnels) with its local stdio profile, setting the MCP command to the absolute `scripts/mcp.sh` path. Follow the official tunnel setup/login instructions, then keep `tunnel-client run` running. In ChatGPT, enable developer mode in Settings → Security and login, add a Plugin connection using **Tunnel**, and select that tunnel. Availability depends on account and workspace policy. See [Connect and test](https://developers.openai.com/plugins/deploy/connect-chatgpt).

This repo supplies the MCP server and native bridge. It does not create an OpenAI tunnel, store account credentials, or automatically install a ChatGPT connection. No OpenAI API call or API key is needed by Pachiri itself. A stdio process alone cannot be reached from ChatGPT's cloud without a supported transport such as the tunnel.

## Tools

- `open_editor`: bring the connected native app forward.
- `get_document`: dimensions, revision, backdrop, editable marks and current IDs.
- `import_image`: exactly one local `path`, image `base64`, or `clipboard: true`. Replaces the current document.
- `add_annotation`, `update_annotation`, `move_annotation`, `delete_annotation`: editable pen, arrow (including quadratic curve), box, text, highlight, pixelate, counter objects.
- `crop_image`, `resize_image`, `set_backdrop`, `undo`, `redo`: native document edits.
- `read_image`: rasterize the current canvas into model-visible PNG content, optionally at animation `phase` 0..1. Preview maximum edge defaults to 1600; never upscales.
- `export_png`: full resolution image to a new local file.
- `export_mp4`: animated background with fixed foreground, H.264/30fps, 2–15 seconds, max edge 1920. Requires a motion backdrop.
- `read_video_frame`: rasterize an MP4 at a timestamp into model-visible PNG content using AVFoundation. No FFmpeg dependency.

Example workflow:

1. Import an image from local disk or clipboard, then call `get_document`.
2. Add an arrow:
   ```json
   {"mark":{"tool":"arrow","points":[[100,100],[400,250]],"curve":[260,60],"color":[255,56,100,255],"width":5,"text":""}}
   ```
3. Add text using one point, `tool: "text"`, and the label in `text`. Font size is `width × 7` pixels.
4. Move/delete/update using an ID returned by the latest state. IDs include a revision and expire after every edit. Optional `expected_revision` rejects stale mutations.
5. Set a moving backdrop:
   ```json
   {"backdrop":{"motion":"lava","preset":0,"padding":100,"seconds":5,"inner_radius":18,"shadow":24}}
   ```
6. Call `read_image` to inspect, `export_mp4` to encode, and `read_video_frame` with the returned path and `seconds: 2.5` to inspect a video frame.

Local paths refer to the Mac. ChatGPT upload/file IDs are not native file paths; the client must supply image bytes as base64 or stage the image locally. There is no automatic ChatGPT attachment download integration in this prototype. PNG image tool responses are inline; MP4 exports return a **local path**, not a cloud-downloadable attachment.

## Behavior and limits

Heavy operations run on the IPC worker, not the UI thread. It snapshots the document and applies successful edits on GPUI's thread only if the revision is unchanged and no manual gesture/text edit is in progress. Native undo history is preserved. The newest object is selected for direct manual editing. Requests are serialized; a video export can delay the next MCP call while the native window remains responsive.

The Unix socket is in `~/Library/Caches/dev.benv.pachiri/automation/editor.sock`, inside a mode-0700 directory, with mode-0600 socket access. This is local-account access, not isolation from other processes running as you. The bridge does not listen on a TCP port. Exports default to the private `automation/exports` folder; explicit output paths must be absolute and existing files are never overwritten.

Images imported through MCP are limited to 16 MiB encoded / 32 megapixels. Drawing schemas and bounds are validated. The bridge refuses edits against stale revisions and unfinished manual operations. Import starts a new document and discards its previous undo history; export/edit tools do not prompt for save dialogs.

## Verification

`cargo test --locked` covers MCP discovery, schemas, import/edit/read-back, arrow geometry, undo, stale IDs, output-file protection, and GPUI-thread application/conflict handling on GPUI's virtual platform. `cargo test --release --locked native_mcp_video_roundtrip -- --ignored --nocapture` additionally exercises the built encoder/decoder against real MP4 and PNG files. Native desktop visibility and ChatGPT account/tunnel connection require a manual acceptance pass.

### Focus tools and GIF export

`add_annotation` / `update_annotation` also accept `spotlight` and `magnifier` marks. Spotlight uses two opposite rectangle corners. Magnifier uses two points: source center and lens center; its radius is `width × 12` pixels (18–300), and `text` specifies zoom (1.5–4, default 2). Source and lens can be moved independently by replacing the mark's points. These objects support native selection, delete, and undo.

`export_gif` uses the same moving backdrop and output-path rules as `export_mp4`, returning `image/gif`, local path, duration and fps. GIFs repeat indefinitely at 20 fps, max edge 960. The palette is learned across one cycle and reused for every frame. The native bridge must be restarted on the new build to use new tool/object types.
