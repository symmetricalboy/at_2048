# Contributing to at://2048

Want to implement a feature or help solving that nasty bug? You are at the
right page - this document will guide you through setting up everything you may
need to get this project up and running.

## So, how do I run it locally?

- Clone this project using your preferred tools:

```bash
# Clone using SSH (the most secure way)
git clone git@github.com:fatfingers23/at_2048.git

# Clone using GitHub CLI (the most convenient way)
gh repo clone fatfingers23/at_2048

# Clone using HTTPS
https://github.com/fatfingers23/at_2048.git
```

- [Install Rust](https://www.rust-lang.org/tools/install), if you haven't already
- Add Wasm target: `rustup target add wasm32-unknown-unknown`
- [Install npm](https://nodejs.org/en/download)
- Install project dependencies using `npm` and `cargo`:
```bash
cargo install trunk wasm-bindgen-cli
npm install tailwindcss daisyui
```
- Go to the `app_2048` directory: `cd at_2048/app_2048`
- Run `trunk serve`

Now, wait for the rest of dependencies to load, compile, and build, and once
it's done, you'll see something like this in your terminal:

```
Done in 151ms
2025-05-05T18:05:39.718398Z  INFO applying new distribution
2025-05-05T18:05:39.720532Z  INFO ✅ success
2025-05-05T18:05:39.720588Z  INFO 📡 serving static assets at -> /
2025-05-05T18:05:39.720771Z  INFO 📡 server listening at:
2025-05-05T18:05:39.720777Z  INFO     🏠 http://127.0.0.1:8080/
2025-05-05T18:05:39.720782Z  INFO     🏠 http://[::1]:8080/
2025-05-05T18:05:39.720864Z  INFO     🏠 http://localhost.:8080/
```

Visit http://localhost:8080, traverse the project files, and go on, do some
hacking!

## I've done something cool and would like to share. How can I?

Go to this repository's page, [fork it](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/working-with-forks/fork-a-repo?tool=webui&platform=linux#forking-a-repository),
save your changes using `git commit`, then push it:
```bash
git remote add github git@github.com:<YOUR_USERNAME>/at_2048.git # only needed once
git push github <YOUR_BRANCH_NAME>
```
After that, come back to this repository and GitHub will propose you to open
the pull request. Do it!
