# wee-slack — wave 1

| rule | fp class | count | example key |
|---|---|---|---|
| #1 | Any forced by the hand-rolled asyncio (invariant Task[T], coroutine yield/return) | 4 | `slack/task.py:190:1:weak:slack.task.process_ended_task:task` |
| #1 | Any is a decoded-JSON value in a dev-only codegen script | 1 | `generate_types_from_mocks.py:16:1:weak:generate_types_from_mocks.generate:body` |
| #6 | fetch_* counted as an accessor prefix, so the I/O the name promises reads as a lie | 22 | `slack/slack_api.py:92:6:dishonest-accessor:slack.slack_api.SlackEdgeApi.fetch_usergroups_info` |
| #6 | 'io' is a read of the store the function is named for | 1 | `extract_token_from_browser.py:45:6:dishonest-accessor:extract_token_from_browser.get_cookies` |
| #6 | the mutation is the callee's own private read-bookkeeping flag | 1 | `slack/util.py:35:6:dishonest-accessor:slack.util.get_resolved_futures` |
| #6 | effect imported from a same-named function in another module | 1 | `slack/util.py:65:6:dishonest-accessor:slack.util.get_cookies` |
| #8 | recurring *_id: str with no validation to lift; the ids are born str in stub TypedDicts | 3 | `slack/slack_api.py:243:8:primitive:conversation_id` |
| #8 | a None-state guard read as a validation predicate | 1 | `slack/slack_workspace.py:652:8:validation:_P_self._ws is None` |
| #9 | the plugin's one deliberate service-locator singleton (weechat dispatches callbacks by global name) | 1 | `slack/shared.py:51:9:shared-state:slack.shared.shared` |
| #10 | widening a module-internal helper whose only two callers pass the same List | 1 | `slack/commands.py:366:10:over-constrained:slack.commands.get_conversation_from_args:args` |
| #11 | typed API facade: identical bodies whose only variation is the endpoint literal and its response TypedDict | 17 | `slack/slack_api.py:182:11:clone:a1de63dc4c65` |
| #11 | repeated read of one config option's dotted path | 1 | `slack/slack_conversation.py:492:11:expr-clone:b10f0cc467ec` |
| #13 | a callback-signature adapter, not a forward | 2 | `slack/config.py:158:13:shallow:slack.config.SlackConfigSectionColor.config_change_buflist_muted_conversation_cb` |
| #13 | a one-expression helper that names a concept (or owns a global's only write) and is reused | 2 | `slack/error.py:92:13:shallow:slack.error.format_exception_only_str` |
| #14 | a weechat callback ABI parameter list the repo does not choose | 3 | `slack/commands.py:760:14:clump:buffer,command,data` |
| #14 | the decorator-imposed handler signature, already named as InternalCommandCallback | 1 | `slack/commands.py:191:14:clump:args,buffer,options` |
| #14 | url/options/timeout, the natural argument list of an HTTP call, forwarded down one chain | 1 | `slack/http.py:65:14:clump:options,timeout,url` |
| #15 | 'options' is a slot of the fixed command-handler dispatch signature | 3 | `slack/commands.py:418:15:wallet:slack.commands.command_slack_join:options` |
| #16 | the mutated dict is built and returned inside the frame; no external state is written | 1 | `slack/slack_emoji.py:43:16:mutation-tail:slack.slack_emoji.load_standard_emojis` |
| #17 | attribute-assignment __init__ with no locals at all, so every line is a zero-crossing neck | 7 | `slack/config.py:43:17:liveness-neck:slack.config.SlackConfigSectionColor.__init__:43` |
| #17 | the neck is a guard preamble or a step boundary in a linear init sequence | 2 | `slack/slack_search_buffer.py:188:17:liveness-neck:slack.slack_search_buffer.SlackSearchBuffer.search:188` |
| #21 | a call to the class's own helper/method/predicate, or a read of its own field | 9 | `slack/slack_api.py:74:21:invariant:slack.slack_api.SlackEdgeApi:e532a51c` |
| #21 | an absent-state guard (buffer_pointer / _ws is None) no Python type can carry | 4 | `slack/slack_conversation.py:150:21:invariant:slack.slack_conversation.SlackConversation:ae068c05` |
| #22 | a cohesive method reading self's public API; the rule's own 5% score says there is no free-function cluster | 2 | `slack/slack_message.py:626:22:velcro:slack.slack_message.SlackMessage.priority_notify_tag` |
| #23 | score driven by an inherently nested or flat-dispatch shape that still reads top-down | 6 | `slack/commands.py:102:23:cognitive-complexity:slack.commands.weechat_command` |
| #23 | at the threshold (15) on a linearly readable body | 2 | `slack/http.py:114:23:cognitive-complexity:slack.http.http_request` |
| #24 | a deliberate eval debug command / a test-only stub loader | 2 | `slack/commands.py:851:24:dynamic-id:eval:851` |
| #24 | a forwarded literal name mirroring a declared same-named attribute | 1 | `slack/config.py:461:24:dynamic-id:getattr:461` |
| #25 | a weechat config-change callback named for its option, delegating to the effect it triggers | 1 | `slack/config.py:158:25:rename-delegation:slack.config.SlackConfigSectionColor.config_change_buflist_muted_conversation_cb` |
| #27 | price: the hot symbol is the module's own root class, so there is nothing to lift out | 5 | `slack/config.py:1:27:price:slack.config` |
| #27 | fan-out that measures layer position or sits at the threshold with a type-only import | 2 | `slack/commands.py:1:27:fan-out:slack.commands` |
| #29 | cost-docstring where heaviness was inferred from line count on a body with no hidden cost | 24 | `slack/commands.py:102:29:cost-docstring:slack.commands.weechat_command` |
| #29 | top-loading on a small module (<=370 lines) whose name already is the map | 7 | `slack/completions.py:1:29:top-loading:slack.completions` |
| #30 | the terminal .value of a WeeChatOption counted as a hop, inflating every config read to 4 | 11 | `slack/slack_api.py:69:30:demeter:slack.slack_api.SlackApiCommon._get_request_options:4` |
| #30 | self.workspace.my_user.<field>: two hops through named domain objects | 9 | `slack/slack_conversation.py:202:30:demeter:slack.slack_conversation.SlackConversation.__init_async:3` |
| #30 | miscount: the terminal segment is an invoked method, not a structural hop | 3 | `slack/slack_search_buffer.py:197:30:demeter:slack.slack_search_buffer.SlackSearchBuffer.search:3` |
| #30 | a view/adapter class projecting through the parent it exists to project | 2 | `slack/slack_message.py:239:30:demeter:slack.slack_message.PendingMessageItem.resolve:3` |
| #32 | the unread 'data' slot of a weechat callback ABI | 1 | `slack/completions.py:294:32:dead-param:slack.completions.input_complete_cb:data` |
| #33 | @x.setter counted as a value-returning boundary | 4 | `slack/slack_message.py:461:33:mixed-returns:slack.slack_message.SlackMessage.last_read` |
| #33 | the standard find-or-None search idiom in a three-line loop | 1 | `slack/slack_message.py:543:33:mixed-returns:slack.slack_message.SlackMessage._get_reaction` |
| #36 | Unknown traced to typeshed's platform-gated 'resource' module on a Windows analysis host | 1 | `slack/http.py:16:36:any-laundering:slack.http.available_file_descriptors` |
| #36 | four casts of one C-API bridging function; nothing else in the module is silenced | 1 | `slack/weechat_config.py:80:36:type-lies:slack.weechat_config` |
| #39 | a vendored more-itertools helper carrying its upstream docstring under an attribution comment | 2 | `slack/util.py:78:39:comment-ratio:slack.util.take` |
| #40 | plural name returning a serialized collection (comma-joined weechat tags, one Cookie header) | 2 | `slack/slack_message.py:644:40:naming-proxy:slack.slack_message.SlackMessage.tags` |
| #44 | x == x / x >= x where the custom operator is the system under test, so the expression is not call-free | 3 | `tests/test_slackts.py:17:44:tautology:tests.test_slackts.test_slackts_eq:17` |
| #48 | a one-statement helper whose name is the only explanation of a cryptic expression | 1 | `extract_token_from_browser.py:32:48:fold:extract_token_from_browser.AESCipher._unpad` |
| #49 | the default's declared type is Mapping, so the checker already forbids mutating it | 3 | `slack/slack_api.py:79:49:mutable-default:slack.slack_api.SlackEdgeApi._fetch_edgeapi:params` |
| #50 | SlackApi fetch_* family: the type is named on the body's second line and the honest return is the narrowed *SuccessResponse arm | 36 | `slack/slack_api.py:92:50:unannotated:slack.slack_api.SlackEdgeApi.fetch_usergroups_info` |
| #50 | a one-to-five-line body whose callee, literal or constructor is the return type | 25 | `generate_types_from_mocks.py:41:50:unannotated:generate_types_from_mocks.ast_equal` |
| #50 | @x.setter counted as an unannotated value-returning boundary | 7 | `slack/slack_conversation.py:270:50:unannotated:slack.slack_conversation.SlackConversation.last_read` |
| #55 | positional order mirroring the weechat API call the body forwards | 1 | `slack/completions.py:32:55:positional-width:slack.completions.completion_list_add_expand` |
| #55 | a weechat callback ABI signature | 1 | `slack/config.py:478:55:positional-width:slack.config.config_section_workspace_read_cb` |

## wave 2

| rule | fp class | count | example key |
|---|---|---|---|
| none | | | |
