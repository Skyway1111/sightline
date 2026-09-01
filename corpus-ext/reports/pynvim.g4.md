# pynvim — wave 1

| rule | fp class | count | example key |
| --- | --- | --- | --- |
| 1 | msgpack-RPC return: the value is whatever nvim/vimscript/lua returns, so `Any` is the exact published contract | 6 | pynvim/api/nvim.py:329:1:weak:pynvim.api.nvim.Nvim.eval:return |
| 1 | callback / arbitrary-callable param whose args are decoded msgpack and whose return is ignored; `Callable[..., Any]` is the only expressible type | 8 | pynvim/api/nvim.py:469:1:weak:pynvim.api.nvim.Nvim.async_call:fn |
| 2 | dunder protocol method: `in` passes any object at runtime, so the annotation cannot discharge the isinstance guard | 1 | pynvim/api/nvim.py:521:2:redundant:isinstance |
| 6 | effects attributed transitively from a shared RPC helper (`request`'s `kwargs.pop`, session bookkeeping), invisible at the accessor's own boundary | 1 | pynvim/api/nvim.py:365:6:dishonest-accessor:pynvim.api.nvim.Nvim.list_runtime_paths |
| 14 | the recurring group is a public decorator's keyword surface; a parameter object would break every call site | 1 | pynvim/plugin/decorators.py:46:14:clump:allow_nested,eval,name,sync |
| 24 | `exec`/`eval` of user-supplied code in the module that *implements* vim's `:python`/`:pyfile`/`:pydo`/`pyeval` | 4 | pynvim/plugin/script_host.py:97:24:dynamic-id:exec:97 |
| 24 | `exec` of a constant literal string — the identifier is greppable, nothing is constructed | 3 | pynvim/plugin/script_host.py:34:24:dynamic-id:exec:34 |
| 24 | attribute-family copy loop over `dir(obj)` by prefix; names are discovered and copied verbatim, declared literally elsewhere | 2 | pynvim/plugin/host.py:276:24:dynamic-id:setattr:276 |
| 26 | deprecated alias/transition package whose contract is "whatever the sibling exports"; a literal list would duplicate and drift | 4 | neovim/__init__.py:8:26:dynamic-all:neovim |
| 26 | regex alternation joined from a literal list read as a member declaration | 1 | scripts/logging_statement_modifier.py:60:26:computed-declaration:logging_statement_modifier.STR_RE_LOGGING_CALL |
| 29 | the file's first screen is a header comment block naming exactly what it is; only the form (comment vs docstring) differs | 1 | docs/conf.py:1:29:top-loading:conf |
| 32 | published API of a client library: autodocumented class members whose callers are external plugins, not this repo | 7 | pynvim/api/nvim.py:313:32:dead-symbol:pynvim.api.nvim.Nvim.subscribe |
| 32 | importlib finder protocol method the interpreter calls through a path hook | 1 | pynvim/plugin/script_host.py:223:32:dead-symbol:pynvim.plugin.script_host.path_hook.VimPathFinder.find_spec |
| 35 | module-level import reported as function-scope; the file has no function-scope import at all | 3 | pynvim/msgpack_rpc/session.py:11:35:hoistable-import:pynvim.msgpack_rpc.session:from pynvim.compat import check_async |
| 36 | sparse pragmas (4/611, 3/313), each a targeted platform or private-attribute silence, called dense | 2 | pynvim/api/nvim.py:155:36:type-lies:pynvim.api.nvim |
| 39 | comment-history anchored at a docstring line where no comment exists and no history is narrated | 1 | pynvim/api/nvim.py:374:39:comment-history:pynvim.api.nvim:374 |
| 48 | the callee is also passed by reference as a callback; an attribute load was not counted as a reference | 2 | pynvim/msgpack_rpc/session.py:231:48:fold:pynvim.msgpack_rpc.session.Session._enqueue_request |
| 55 | `@overload` stubs are the published contract (three positional slots, `*` where needed); the implementation signature is not a boundary | 1 | pynvim/__init__.py:99:55:positional-width:pynvim.attach |
