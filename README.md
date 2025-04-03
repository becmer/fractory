<!--suppress HtmlDeprecatedAttribute -->
<table>
<tr>
<td align="right">
  <b>Support Fractory:</b>
</td>
<td>
  <a href="https://github.com/sponsors/becmer">GitHub Sponsors</a> •
  <a href="https://www.buymeacoffee.com/becmer">Buy Me a Coffee</a> •
  <a href="https://ko-fi.com/becmer">Ko-fi</a>
</td>
</tr>
<tr>
<td align="right">
  <b>Read Manifesto:</b>
</td>
<td>
  <a href="https://medium.com/@kamil.becmer/i-dont-have-lifetime-for-your-bullshit-cdaed08d0646">I don’t have lifetime for your bullshit</a>
</td>
</tr>
</table>

<p>
  <a href="https://crates.io/crates/fractory"><img src="https://img.shields.io/crates/v/fractory.svg" alt="crates.io"></a>
  <a href="https://docs.rs/fractory"><img src="https://docs.rs/fractory/badge.svg" alt="docs.rs"></a>
  <a href="https://github.com/becmer/fractory/actions"><img src="https://github.com/becmer/fractory/actions/workflows/build.yml/badge.svg" alt="Build status"></a>
  <a href="https://github.com/becmer/fractory/blob/main/LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="license: MIT OR Apache-2.0"></a>
  <img src="https://img.shields.io/badge/status-WIP-orange" alt="Project status: WIP">
</p>

# Fractory

**Fractory** is a Rust-based tool that tracks file changes and reconstructs development sessions — built for mosaic thinkers who thrive on context, not checklists.

It helps you preserve flow, capture intent, and navigate your work history as a living, nonlinear timeline. Whether you’re deep in systems design, refactoring, or jumping between ideas, Fractory ensures nothing gets lost in the chaos.

> **⚠️ Warning**: This project is in early development. The current release is a placeholder to claim the crate name.

---

## 💡 Why Fractory?

Fractory exists for developers who think in fragments, not funnels — who build systems nonlinearly, jump between layers, and resist rigid workflows.
It preserves flow, captures context, and lets you reconstruct what you were really doing — even if you couldn’t explain it in a ticket.

📝 Read the full manifesto: [**I don’t have lifetime for your bullshit**](https://medium.com/@kamil.becmer/i-dont-have-lifetime-for-your-bullshit-cdaed08d0646)

---

## ✨ Features (WIP)

- ⏱️ Session detection based on idle time, commits, or manual markers
- 🧠 GPT-powered summaries of your coding sessions
- 🧩 Intent annotation via CLI or Git hooks
- 🧾 Lightweight, `.gitignore`-aware diff tracking
- 🧵 `fractory resume` brings you right back into flow

---

## 🚀 Getting Started

```sh
cargo install fractory
fractory init        # Initializes project baseline
fractory start       # Begins tracking changes
fractory summary     # Generates a summary of your session
```

---

## 🌱 Built for Mosaic Thinking

Fractory is for developers who:
- See systems holistically, not step-by-step
- Jump between files, ideas, and problems
- Lose flow when forced into linear workflows

---

## 📦 License

Fractory is licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.

SPDX identifier: `MIT OR Apache-2.0`
