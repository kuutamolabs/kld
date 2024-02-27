🌔kuutamo lightning-knd TUI
---

A Terminal User Interface for [lightning-knd](https://github.com/kuutamolabs/lightning-knd).

Now you can try with previous version of app with `--sync` flag for all commands.

Current asynchronous app is **still under development** and already supports
default [keybinding](https://github.com/kuutamolabs/lightning-tui/blob/non-blocking/assets/keybinding.toml) is here
You can copy `assets/vim_keybinding.toml` or create anyone `keybinding.toml` in your working directory to overwrite any key bindings.

- [x] non-blocking architecture
- [x] log features
- [x] multiple mode key bindings
- [x] allow user customized key bindings
- [x] i18n support based on `LANG` setting
- [x] command list
- [x] add action inspector in debug component
- [x] helper page
- [x] command history
- [x] prompt if the command is not ready

We will do following items later.

- [ ] reimplement each command in non-blocking way
  - [x] Node information
	- [ ] NodeFees,
	- [ ] NodeEslq,
	- [ ] NodeSign,
	- [ ] NodeLsfd,
	- [ ] NetwLsnd,
	- [ ] NetwFeer,
	- [ ] PeerList,
  - [x] Connect Peer
	- [ ] PeerDisc,
	- [ ] PaymList,
	- [ ] PaymSdky,
	- [ ] PaymPayi,
	- [ ] InvoList,
	- [ ] InvoGene,
	- [ ] InvoDeco,
	- [ ] ChanList,
	- [x] Open Channel
	- [ ] ChanSetf,
	- [ ] ChanClos,
	- [ ] ChanHist,
	- [ ] ChanBala,
	- [ ] ChanLsfd,
