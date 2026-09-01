| rule | fp class | count | example key |
|---|---|---|---|
| 9 | network-backed classmethod stubbed at an SDK boundary; already parameterised, patches vestigial (no live prod call path) | 1 | djstripe/models/account.py:154:9:test-writers:djstripe.models.account.Account.get_default_account |
| 9 | template-hook method override patched out for fixture shape (plain dict lacks auto_paging_iter); inputs already parameters | 1 | djstripe/models/connect.py:263:9:test-writers:djstripe.models.connect.Transfer._attach_objects_post_save_hook |
