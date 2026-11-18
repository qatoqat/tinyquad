// WebAudio output for the rainy_night example (Rust side: audio/web.rs).
//
// Include AFTER gl.js and BEFORE the load("....wasm") call:
//
//     <script src="gl.js"></script>
//     <script src="audio.js"></script>
//     <script>load("rainy_night.wasm");</script>
//
// Browsers only let a page make sound after a user gesture, so nothing happens until the
// first key press / click / touch. Then an AudioContext is created and a ScriptProcessorNode
// pulls interleaved stereo samples out of the wasm module:
//
//     rn_audio_init(sample_rate: f32)
//     rn_audio_render(frames: u32) -> *const f32   (frames * 2 floats in wasm memory)
//
// ScriptProcessorNode is deprecated but universally supported, and unlike an AudioWorklet it
// runs on the main thread - which is exactly where the (single-threaded) wasm module lives,
// so no SharedArrayBuffer / cross-origin isolation / message ring buffer is needed.
"use strict";

(function () {
    // ~43 ms at 48 kHz: big enough to survive a slow frame, small enough that a splash
    // still feels attached to what is on screen.
    var BUFFER_FRAMES = 2048;
    var GESTURES = ["keydown", "pointerdown", "mousedown", "touchstart"];

    var ctx = null;
    var node = null;
    var ready = false; // wasm instantiated (on_init has run)

    function process(event) {
        var out = event.outputBuffer;
        var left = out.getChannelData(0);
        var right = out.numberOfChannels > 1 ? out.getChannelData(1) : null;
        var frames = out.length;
        var ptr = wasm_exports.rn_audio_render(frames);
        // Make the view on every call: the ArrayBuffer is replaced whenever wasm memory grows.
        var samples = new Float32Array(wasm_memory.buffer, ptr, frames * 2);
        if (right) {
            for (var i = 0; i < frames; i++) {
                left[i] = samples[2 * i];
                right[i] = samples[2 * i + 1];
            }
        } else {
            for (var j = 0; j < frames; j++) {
                left[j] = 0.5 * (samples[2 * j] + samples[2 * j + 1]);
            }
        }
    }

    function unlock() {
        if (!ready || wasm_exports.rn_audio_render == undefined) {
            return; // not loaded yet (or audio not compiled in); try again on the next gesture
        }
        if (ctx == null) {
            var AudioCtx = window.AudioContext || window.webkitAudioContext;
            if (AudioCtx == undefined) {
                remove_listeners();
                return;
            }
            try {
                ctx = new AudioCtx({ latencyHint: "interactive" });
                wasm_exports.rn_audio_init(ctx.sampleRate);
                node = ctx.createScriptProcessor(BUFFER_FRAMES, 0, 2);
                node.onaudioprocess = process;
                node.connect(ctx.destination);
            } catch (e) {
                console.warn("rn_audio: audio disabled:", e);
                ctx = null;
                remove_listeners();
                return;
            }
        }
        if (ctx.state != "running") {
            // Safari starts suspended even inside a gesture; resume() returns a promise.
            var p = ctx.resume();
            if (p && p.catch) {
                p.catch(function () { });
            }
        }
        if (ctx.state == "running") {
            remove_listeners();
        }
    }

    function remove_listeners() {
        GESTURES.forEach(function (name) {
            window.removeEventListener(name, unlock, true);
        });
    }

    function register_plugin(importObject) {
        // The Rust side imports nothing: JS pulls, Rust never calls out.
    }

    function on_init() {
        ready = true;
        // Capture phase, so we still see the event when the game calls preventDefault().
        GESTURES.forEach(function (name) {
            window.addEventListener(name, unlock, true);
        });
        // Do not keep the audio clock (and the synth) running in a hidden tab.
        document.addEventListener("visibilitychange", function () {
            if (ctx == null) {
                return;
            }
            if (document.hidden) {
                ctx.suspend();
            } else {
                ctx.resume();
            }
        });
    }

    miniquad_add_plugin({
        register_plugin: register_plugin,
        on_init: on_init,
        name: "rn_audio",
        version: 1
    });
})();
