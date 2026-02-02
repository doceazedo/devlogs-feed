# devlogs-feed

A custom Bluesky feed with ML-curated gamedev content that boosts organic human posts and penalizes self-promo.

## Usage

This feed is [available here](https://bsky.app/profile/doceazedo.com/feed/devlogs-feed). Click on the "📌" icon to pin it, and click on the feed name, then on the "❤️ Like" button to like it!

## Running locally

You will need [Rust toolchain](https://rust-lang.org/tools/install/) and Diesel CLI (`cargo install diesel_cli`) installed.

### Setup

```bash
cp .env.example .env
```

```bash
diesel setup
diesel migration run
```

### Run

```bash
cargo run
```

### Test scoring

```bash
cargo run --bin score-post "i just finished implementing the combat system! #gamedev"

cargo run --bin score-post https://bsky.app/profile/[...]/post/[...]
```

## Acknowledgments

Built with [skyfeed](https://github.com/cyypherus/skyfeed) by [@cyypherus](https://github.com/cyypherus).

## License

This project is licensed under the [GNU GPLv3 license](LICENSE).
