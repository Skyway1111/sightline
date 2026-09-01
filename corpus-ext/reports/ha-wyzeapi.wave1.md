# ha-wyzeapi — wave 1

Repo: `<GAUNTLET_CORPUS_ROOT>\ha-wyzeapi`
Prod tree judged: `custom_components/wyzeapi/` (15 modules, 3,706 lines) plus
`pyproject.toml` / `manifest.json` / `strings.json` where a prod symbol points at them.
Blind: no sightline output consulted.

## Phase 1 — blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | custom_components/wyzeapi/__init__.py:70 | #2 | Dead branch: line 57 already returns `True` whenever `async_entries(DOMAIN)` is truthy, so this identical call is provably falsy here and lines 70-79 are unreachable. | `if hass.config_entries.async_entries(DOMAIN):` |
| P1-2 | custom_components/wyzeapi/__init__.py:53 | #1 | `discovery_info` is unannotated (implicit `Any`) and never read; `async_setup` also has no return annotation although it returns `bool`. | `hass: HomeAssistant, config: HomeAssistantConfig, discovery_info=None` |
| P1-3 | custom_components/wyzeapi/__init__.py:140 | none | Stores the constant *names* `KEY_ID`/`API_KEY` ("key_id"/"api_key") instead of the credential values read at L106-107 into `key_id`/`api_key`. | `"key_id": KEY_ID,` / `"api_key": API_KEY,` |
| P1-4 | custom_components/wyzeapi/__init__.py:180 | none | `options_update_listener` is never registered — no `add_update_listener` call exists in the package — so option changes never reload the entry. | `async def options_update_listener(hass: HomeAssistant, config_entry: ConfigEntry):` |
| P1-5 | custom_components/wyzeapi/__init__.py:212 | #4 | The `setdefault` default can never fire: the sole caller (L144) initialised `"coordinators": {}` two lines earlier at L142. | `coordinators = hass.data[DOMAIN][config_entry.entry_id].setdefault("coordinators", {})` |
| P1-6 | custom_components/wyzeapi/__init__.py:211 | #8 | The Lock-Bolt model literal `"YD_BT1"` selects a device family at three sites (here, lock.py:58, lock.py:63) with no shared constant, while const.py owns every other protocol literal. | `if lock.product_model == "YD_BT1":` |
| P1-7 | custom_components/wyzeapi/__init__.py:138 | #1 | `hass.data[DOMAIN][entry_id]` is an untyped bag mixing a client, two credential strings and a coordinator map; every consumer re-derives the shape by string key (switch.py:81, lock.py:61). | `hass.data[DOMAIN][config_entry.entry_id] = {CONF_CLIENT: client, ...}` |
| P1-8 | custom_components/wyzeapi/token_manager.py:17 | #9 | `hass`/`config_entry` are class attributes written through the class in `__init__` (L21-22), so every instance overwrites one process-wide global that the `@staticmethod` callback then reads. | `hass: HomeAssistant = None` / `config_entry: ConfigEntry = None` |
| P1-9 | custom_components/wyzeapi/token_manager.py:27 | #3 | Emptiness guard around a `for` over the same call: an empty list already skips the loop, and the guard costs a second `async_entries` call. | `if TokenManager.hass.config_entries.async_entries(DOMAIN):` |
| P1-10 | custom_components/wyzeapi/token_manager.py:25 | #1 | `token: Token = None` declares a non-Optional parameter with a `None` default while L34 dereferences `token.access_token` unconditionally. | `async def token_callback(token: Token = None):` |
| P1-11 | custom_components/wyzeapi/token_manager.py:42 | none | The decorator discards the wrapped function's return value (no `return`) and omits `functools.wraps`, so every decorated coroutine — e.g. `lock.py:257 async_lock` — returns `None` and loses its name/docstring. | `if iscoroutinefunction(func): await func(*args, **kwargs)` |
| P1-12 | custom_components/wyzeapi/token_manager.py:1 | #29 | No module docstring on the module owning auth-token refresh and the package-wide exception decorator (same gap: coordinator.py:1, ydble_utils.py:1). | `import logging` |
| P1-13 | custom_components/wyzeapi/coordinator.py:139 | #1 | `context: Dict` is a bare unparameterised `typing.Dict` used as the BLE handshake state machine, keyed by the string literals `"stage"`, `"command"`, `"l1_unfinished"`. | `self, sender, data: bytearray, client: BleakClient, context: Dict` |
| P1-14 | custom_components/wyzeapi/coordinator.py:141 | #18 | `_handle_uart_rx` narrates its phases in prose across a four-stage cascade: `# Process for unfinished data` (L141), `# Process messages` (L150), `# Ack for request chanllenge` (L152), `# Got generated chanllenge` (L161), `# Ack for send_lock_unlock` (L168). | `# Process for unfinished data` |
| P1-15 | custom_components/wyzeapi/coordinator.py:109 | #11 | `_request_challenge`, `_send_lock_unlock` (L114) and `_send_ack` (L121) are the same three-line body — build L2 payload, `pack_l1`, `write_gatt_char(YDBLE_UART_TX_UUID, req, response=False)`. | `req = pack_l1(0, 1, l2_content)` / `await client.write_gatt_char(YDBLE_UART_TX_UUID, req, response=False)` |
| P1-16 | custom_components/wyzeapi/coordinator.py:91 | #11 | The `_get_ble_client()` None-check plus the "Could not find BLE device {nickname} with address {mac}" message is duplicated verbatim from L68-72, differing only in the exception class raised. | `client = await self._get_ble_client()` / `if client is None:` |
| P1-17 | custom_components/wyzeapi/coordinator.py:88 | none | Raises bare `Exception` (also L93) on a user-facing lock command; every other platform raises `HomeAssistantError`, which HA renders to the user. | `raise Exception(f"Waiting for {self._current_command} command to complete")` |
| P1-18 | custom_components/wyzeapi/coordinator.py:98 | none | Fire-and-forget task with no retained reference: the delayed disconnect can be garbage-collected before it runs (same pattern at camera.py:317). | `asyncio.create_task(self._disconnect(delay=10))` |
| P1-19 | custom_components/wyzeapi/coordinator.py:85 | #1 | `command` is unannotated and stringly-typed; it flows unchecked into `ydble_utils.pack_l2_lock_unlock` (L71) where the accepted domain is enforced by string compare at runtime. | `async def lock_unlock(self, command="lock"):` |
| P1-20 | custom_components/wyzeapi/ydble_utils.py:109 | #26 | The 256-entry CRC table is declared as a 1,792-char hex blob and then assembled by a comprehension: the reader cannot read the table, only the code that builds it. | `magic = [int.from_bytes(binascii.unhexlify(magic[x : x + 4])) for x in range(0, len(magic), 4)]` |
| P1-21 | custom_components/wyzeapi/ydble_utils.py:90 | none | That table rebuild lives *inside* `crc()`, so 256 `unhexlify` calls and a 256-element list build run on every packet; `crc` is called from `pack_l1` (L22) and twice per `parse_l1` (L42-43). | `def crc(data):` |
| P1-22 | custom_components/wyzeapi/ydble_utils.py:42 | none | `crc(l2_content)` is computed twice on the failure path — once in the condition, once to format the message. | `if len(l2_content) == length and crc(l2_content) != data_crc:` |
| P1-23 | custom_components/wyzeapi/ydble_utils.py:32 | #1 | `parse_l1` (and `parse_l2_dict`, L57) return unannotated positional tuples that callers destructure by position at coordinator.py:145 and 159; the fourth element is a remaining-byte count with no name. | `return l2_content, flags, seq_no, length - len(l2_content)` |
| P1-24 | custom_components/wyzeapi/ydble_utils.py:14 | #11 | `encrypt_ecb` is `decrypt_ecb` (L7) with one method name changed — four identical lines including the pointless intermediate local. | `cipher = AES.new(key_bytes, AES.MODE_ECB)` / `encrypted_data = cipher.encrypt(data)` |
| P1-25 | custom_components/wyzeapi/ydble_utils.py:71 | #1 | `command` is the only unannotated parameter; its legal domain ("lock"/"unlock") is stated only inside the error string at L77. | `def pack_l2_lock_unlock(ble_id: int, ble_token: str, challenge: bytes, command):` |
| P1-26 | custom_components/wyzeapi/ydble_utils.py:82 | none | Three unnamed protocol frames (L82, L84, L86) are written as bare hex ints with byte widths; nothing says what any field means. | `result = (0x0400050002).to_bytes(5)` |
| P1-27 | custom_components/wyzeapi/config_flow.py:59 | #1 | The annotation uses the builtin function `any`, not `typing.Any` (imported at L6 and used correctly on the next line) — `dict[str, any]` is not a type. | `self, user_input: Optional[dict[str, any]] = None` |
| P1-28 | custom_components/wyzeapi/config_flow.py:45 | #9 | `user_params` is a class-level mutable dict mutated per-flow at L84-87 and L117-119; two concurrent config flows share one credential bag. | `user_params = {}` |
| P1-29 | custom_components/wyzeapi/config_flow.py:44 | #1 | `client: Wyzeapy = None` declares a non-Optional class attribute initialised to `None`; `get_client` (L55) depends on that falsy value. | `client: Wyzeapy = None` |
| P1-30 | custom_components/wyzeapi/config_flow.py:54 | #6 | `get_client` is accessor-named, returns nothing, and exists purely for the side effect `self.client = await Wyzeapy.create()`. | `async def get_client(self):` / `if not self.client:` |
| P1-31 | custom_components/wyzeapi/config_flow.py:79 | none | `except CannotConnect:` can never fire — `CannotConnect` (L177) is raised nowhere in the package, so the `cannot_connect` error string is unreachable. | `except CannotConnect:` |
| P1-32 | custom_components/wyzeapi/config_flow.py:181 | none | `InvalidAuth` is declared and never referenced anywhere in the package. | `class InvalidAuth(HomeAssistantError):` |
| P1-33 | custom_components/wyzeapi/config_flow.py:49 | none | The four instance fields set in `__init__` are never read; the flow stores its state in the class-level `user_params` instead. | `self.email = None` / `self.password = None` |
| P1-34 | custom_components/wyzeapi/config_flow.py:90 | #3 | Emptiness guard around a `for` over the same call, repeated at L120-121; the loop body `return`s on its first iteration, so the guard and the loop are both standing in for "if there is an existing entry". | `if self.hass.config_entries.async_entries(DOMAIN):` |
| P1-35 | custom_components/wyzeapi/config_flow.py:120 | #11 | The reauth-vs-create block here duplicates L90-96 exactly, differing only in the `data=` argument (`self.user_params` vs `user_input`). | `for entry in self.hass.config_entries.async_entries(DOMAIN):` |
| P1-36 | custom_components/wyzeapi/config_flow.py:134 | #13 | `async_step_import` is a single forwarding call that renames one parameter and adds nothing. | `return await self.async_step_user(import_config)` |
| P1-37 | custom_components/wyzeapi/config_flow.py:149 | #15 | `config_entry` is demanded and never touched — L153 constructs `OptionsFlowHandler()` with no arguments. | `def async_get_options_flow(config_entry: config_entries.ConfigEntry) -> OptionsFlowHandler:` |
| P1-38 | custom_components/wyzeapi/config_flow.py:36 | #28 | The 2FA schema keys on `CONF_ACCESS_TOKEN` ("access_token") but `strings.json` declares `config.step.2fa.data.verification_code`; the declared translation key resolves to no schema field, so the input renders untranslated. | `STEP_2FA_DATA_SCHEMA = vol.Schema({CONF_ACCESS_TOKEN: str})` |
| P1-39 | custom_components/wyzeapi/config_flow.py:32 | #28 | Schema keys are `key_id`/`api_key` (const.py:9-10) but `strings.json` `config.step.user.data` declares `keyid`/`apikey` — neither side resolves to the other. | `vol.Required(KEY_ID): str,` / `vol.Required(API_KEY): str,` |
| P1-40 | custom_components/wyzeapi/config_flow.py:142 | #28 | `step_id="reauth_confirm"` names a step that does not exist in `strings.json` (only `user` and `2fa` are declared). | `step_id="reauth_confirm",` |
| P1-41 | custom_components/wyzeapi/siren.py:71 | #1 | Untyped opaque `**kwargs` on a public command method that never reads them (same at L85, which also lacks a return annotation). | `async def async_turn_on(self, **kwargs) -> None:` |
| P1-42 | custom_components/wyzeapi/siren.py:59 | none | `_available: bool` is declared and never assigned or read; `available` (L108) reads the device. Same dead declaration at switch.py:296, 480, 566 and light.py:381. | `_available: bool` |
| P1-43 | custom_components/wyzeapi/siren.py:60 | none | `_just_updated` is written at L81 and L95 and never read — this entity has no `async_update`, unlike switch.py:417 where the identical flag gates a fetch. | `_just_updated = False` |
| P1-44 | custom_components/wyzeapi/siren.py:121 | #11 | The hand-built device-info dict recurs ~20 times across the package (switch.py:241/320/490/576/660, light.py:132/444, sensor.py:204/277/336/471, climate.py:333, lock.py:92/235, cover.py:75, binary_sensor.py:93/158, camera.py:113, fan.py:94) while seven other sites build the same value with the typed `DeviceInfo(...)`. | `"identifiers": {(DOMAIN, self._device.mac)},` / `"manufacturer": "WyzeLabs",` |
| P1-45 | custom_components/wyzeapi/siren.py:50 | #8 | "Which product models support X" is re-encoded as an ad-hoc literal predicate at eight sites (switch.py:53/58/59/100, sensor.py:51/52, button.py:26, light.py:74-94, cover.py:50) with no shared model registry. | `if camera.product_model not in ["WYZECP1_JEF", "WYZEC1-JZ", "GW_BE1"]:` |
| P1-46 | custom_components/wyzeapi/switch.py:477 | #11 | `WyzeCameraNotificationSwitch` (L477-560) and `WyzeCameraMotionSwitch` (L563-646) are 84-line near-identical classes differing only in the service method pair and the `notify`/`motion` attribute. | `class WyzeCameraNotificationSwitch(SwitchEntity):` |
| P1-47 | custom_components/wyzeapi/switch.py:433 | #2 | `isinstance(switch, Camera)` inside a method whose parameter is annotated `switch: Switch` — under the declared type this branch is unreachable, yet it is the entire camera-event path. | `if isinstance(switch, Camera):` |
| P1-48 | custom_components/wyzeapi/switch.py:143 | none | `mac` leaks out of the loop and is read at L146: a device with no identifiers raises `NameError`, one with several silently uses the last. | `for identifier in device.identifiers:` / `mac = identifier[1]` |
| P1-49 | custom_components/wyzeapi/switch.py:405 | #11 | The ip/rssi/ssid attribute block is byte-identical to light.py:296-301 — both carry a `# noinspection DuplicatedCode` marker acknowledging the copy. | `if self._device.device_params.get("ip"):` / `dev_info["IP"] = str(self._device.device_params.get("ip"))` |
| P1-50 | custom_components/wyzeapi/switch.py:401 | none | Concatenates before stringifying: if `electricity` is numeric this raises `TypeError`, and if it is already a string the `str()` is redundant. Each key is also fetched twice. | `dev_info["Battery"] = str(self._device.device_params.get("electricity") + "%")` |
| P1-51 | custom_components/wyzeapi/switch.py:124 | #19 | List membership evaluated per camera inside the loop; the three model exclusion sets in this file are declared as two lists and one set (L53, L58, L59) with no reason for the difference. | `if switch.product_model not in MOTION_SWITCH_UNSUPPORTED:` |
| P1-52 | custom_components/wyzeapi/switch.py:104 | #12 | Builds a one-element list literal to express `==`. | `if switch.product_model in [OUTDOOR_PLUG_INDIVUAL_OUTLETS]:` |
| P1-53 | custom_components/wyzeapi/switch.py:163 | #15 | `config_entry` is used only for `.entry_id` (L175) and never forwarded; `device_list` and `device_registry` are unannotated in the same signature. | `async def async_migrate_switch_data(hass, config_entry, device_list, device_registry)` |
| P1-54 | custom_components/wyzeapi/switch.py:199 | #11 | The device-issue block (L202-217) repeats the entity-issue block (L181-197): same `ir.async_create_issue` call, same URL, two placeholders changed. | `device_automations = automations_with_device(hass, device)` |
| P1-55 | custom_components/wyzeapi/switch.py:649 | none | Class name transposes the vendor name ("Wzye"), defeating grep for `Wyze*Switch`. | `class WzyeLightstripSwitch(SwitchEntity):` |
| P1-56 | custom_components/wyzeapi/switch.py:80 | none | The switch platform announces itself as the light component — copied from light.py:62 and never corrected. | `_LOGGER.debug("""Creating new WyzeApi light component""")` |
| P1-57 | custom_components/wyzeapi/switch.py:51 | none | One fact, three declarations, two shapes: `OUTDOOR_PLUGS` is a `str` here but `["WLPPO"]` at sensor.py:52 and button.py:26, so L100 membership-tests a string against a list of strings. | `OUTDOOR_PLUGS = "WLPPO"` |
| P1-58 | custom_components/wyzeapi/switch.py:446 | #12 | Rebuilds the list on every iteration instead of `list.extend`. | `_ai_tag_list = _ai_tag_list + resource["ai_tag_list"]` |
| P1-59 | custom_components/wyzeapi/sensor.py:490 | #1 | `now: datetime` annotates against the *module* `datetime` (`import datetime`, L4), not `datetime.datetime`; the parameter is also never read. | `async def _async_reset_at_midnight(self, now: datetime) -> None:` |
| P1-60 | custom_components/wyzeapi/sensor.py:161 | #2 | `_enabled` is initialised to `None` (L141) and only ever set to `True`, so `is False` is never satisfied; the guard is dead and the unconditional assignment would be equivalent. | `if self.enabled is False:` / `self.enabled = True` |
| P1-61 | custom_components/wyzeapi/sensor.py:127 | none | A writable `enabled` property (getter here, setter 100 lines away at L232, both above the class constants) shadows Home Assistant's read-only `Entity.enabled`. | `def enabled(self):` / `return self._enabled` |
| P1-62 | custom_components/wyzeapi/sensor.py:155 | #21 | The `self._battery_type == self.<CONST>` discriminator is re-tested in four methods of this class (L155, L159, L195, L226/228): a two-mode class that wants two types. | `if self._lock.raw_dict.get("power") and self._battery_type == self.LOCK_BATTERY:` |
| P1-63 | custom_components/wyzeapi/sensor.py:139 | #1 | Both parameters unannotated on a public constructor (same at L244, `def __init__(self, camera)`), while every sibling sensor annotates its device. | `def __init__(self, lock, battery_type) -> None:` |
| P1-64 | custom_components/wyzeapi/sensor.py:326 | #6 | `__init__` writes an attribute onto the caller's `Switch` object and silences the type error: an effect on foreign state that no signature mentions. | `self._switch.usage_history = None  # type: ignore[attr-defined]` |
| P1-65 | custom_components/wyzeapi/sensor.py:401 | #6 | `update_energy` mutates five instance fields and returns one of them; its only caller (L407) discards the return and reads the mutated field on the next line. | `return self._hourly_energy_usage_added` |
| P1-66 | custom_components/wyzeapi/sensor.py:352 | #18 | `update_energy` labels its phases in prose: `# Handle rolling to the next UTC day` (L352), `# Set inital values to current values on startup` (L363), `# New Hour` (L372), `# Current Hour` (L382). | `if _now == 0:  # Handle rolling to the next UTC day` |
| P1-67 | custom_components/wyzeapi/sensor.py:566 | #11 | `WyzeIrrigationRSSI`, `WyzeIrrigationIP` (L593) and `WyzeIrrigationSSID` (L615) are three ~20-line classes identical but for a display name, a unique-id suffix and one attribute read — a table of three rows written as three classes. | `class WyzeIrrigationRSSI(WyzeIrrigationBaseSensor):` |
| P1-68 | custom_components/wyzeapi/sensor.py:655 | #11 | This `device_info` — `DeviceInfo(...)` plus three conditional enrichments (app_version, sn, wifi_mac) — is duplicated in fan.py:92-108 for the *same physical device*, there as an untyped dict. | `if self._air_purifier.app_version:` / `device_info["sw_version"] = self._air_purifier.app_version` |
| P1-69 | custom_components/wyzeapi/sensor.py:91 | #19 | List membership evaluated per camera inside the comprehension (L97 repeats it with `OUTDOOR_PLUGS`); both are model sets that should be `frozenset`. | `if camera.product_model in CAMERAS_WITH_BATTERIES` |
| P1-70 | custom_components/wyzeapi/sensor.py:224 | #1 | Unannotated property returning `str` on two paths and int `0` on the third, on an entity declared `SensorDeviceClass.BATTERY` with `PERCENTAGE` units. | `def native_value(self):` / `return str(self._lock.raw_dict.get("power"))` |
| P1-71 | custom_components/wyzeapi/light.py:164 | #12 | Wraps a single value in a list to call `any` — the whole expression is `if kwargs.get(ATTR_COLOR_TEMP_KELVIN, kwargs.get(ATTR_HS_COLOR)):`. | `if any([kwargs.get(ATTR_COLOR_TEMP_KELVIN, kwargs.get(ATTR_HS_COLOR))]):` |
| P1-72 | custom_components/wyzeapi/light.py:146 | #17 | 100-line command method with five independent kwarg-driven phases; the live-local set necks down to `options` between each, and `kwargs.get(ATTR_EFFECT)` is re-fetched five times (L206, 207, 218, 224, 230). | `async def async_turn_on(self, **kwargs: Any) -> None:` |
| P1-73 | custom_components/wyzeapi/light.py:218 | #21 | The effect-name-to-code mapping is written forwards here (shadow→"1", leap→"2", flicker→"3") and backwards at L307-313 in `extra_state_attributes` — two homes for one table. | `if kwargs.get(ATTR_EFFECT) == EFFECT_SHADOW:` / `self._bulb.effects = "1"` |
| P1-74 | custom_components/wyzeapi/light.py:490 | none | Returns a scalar where the sibling implementation (L265) and Home Assistant both expect a set of `ColorMode`. | `def supported_color_modes(self):` / `return ColorMode.ONOFF` |
| P1-75 | custom_components/wyzeapi/light.py:110 | #21 | `self._config_entry.options.get(BULB_LOCAL_CONTROL)` is re-derived in four methods (L110, 149, 252, 362) rather than resolved once behind a named accessor. | `self._local_control = config_entry.options.get(BULB_LOCAL_CONTROL)` |
| P1-76 | custom_components/wyzeapi/light.py:393 | none | `_is_on` is assigned at L393, L405 and L419 but never read — `is_on` (L424) reads `self._device.floodlight` directly. | `self._is_on = self._device.floodlight` |
| P1-77 | custom_components/wyzeapi/light.py:105 | #1 | `config_entry` is the only unannotated parameter of this constructor, and the class stores it whole to read one option. | `def __init__(self, bulb_service: BulbService, bulb: Bulb, config_entry) -> None:` |
| P1-78 | custom_components/wyzeapi/light.py:79 | none | Raw dict index on an optional key inside a setup loop (also L86, and cover.py:50): any camera without a dongle raises `KeyError` and aborts the whole platform setup. | `and camera.device_params["dongle_product_model"] == "HL_CFL"` |
| P1-79 | custom_components/wyzeapi/camera.py:96 | #7 | The comment narrates a protocol that no longer exists: `_config_task` (L100) is assigned once and never read or set again, and neither named method awaits it. | `# Always holds an in-flight Task[dict] for the next config fetch.` |
| P1-80 | custom_components/wyzeapi/camera.py:186 | #7 | The precondition — `config_fetch()` must have run, which happens only in `async_setup_entry` (L67) inside a `try/except Exception` that swallows failure — is narrated in a comment instead of encoded. | `# This shouldn't happen, but throw an error if we don't have a config ready yet` |
| P1-81 | custom_components/wyzeapi/camera.py:92 | none | Assigns over `CameraEntity` properties (`supported_features`, and `name` at L88, `model` at L91) instead of the `_attr_*` fields the same constructor uses correctly at L89. | `self.supported_features = CameraEntityFeature.STREAM` |
| P1-82 | custom_components/wyzeapi/camera.py:125 | none | `@cached_property` on live device state: after the first read `is_streaming` never reflects `handle_camera_update` (L131), which is the only thing that refreshes `self._camera`. | `def is_streaming(self) -> bool:` / `return self._camera.on` |
| P1-83 | custom_components/wyzeapi/camera.py:283 | none | `self.camera_service` is set to `None` and never assigned or read again. | `self.camera_service = None` |
| P1-84 | custom_components/wyzeapi/camera.py:332 | none | The KVS recipient client id is a hardcoded literal appearing twice (here and L371) with no name explaining what it is. | `"recipientClientId": "ada06f08-87f4-4e13-b699-e82db8517ae5",` |
| P1-85 | custom_components/wyzeapi/camera.py:369 | #11 | `send_candidate`'s payload construction, base64/json encode, debug log and send (L369-383) repeat `send_offer`'s (L330-345) with only the action name and inner dict changed. | `payload = {"action": "ICE_CANDIDATE", ...}` |
| P1-86 | custom_components/wyzeapi/camera.py:410 | #19 | The answer is re-scanned with `finditer` inside the loop over offers, making the match quadratic in SDP sections for no reason — the answer never changes within an iteration. | `sdp_answers = re.finditer(sdp_pattern, self.sdp_answer)` |
| P1-87 | custom_components/wyzeapi/camera.py:99 | #1 | Bare `dict` for the stream-info contract whose keys (`"ice_servers"`, `"signaling_url"`) are read at L193 and L303; `config: dict` at L278 repeats it across the class boundary. | `self._cached_config: dict \| None = None` |
| P1-88 | custom_components/wyzeapi/camera.py:170 | none | `getattr(..., "motion", None)` plus an `isinstance` probe substitutes for a typed optional on the `Camera` model; the attribute name is invisible to symbol search. | `motion = getattr(self._camera, "motion", None)` |
| P1-89 | custom_components/wyzeapi/climate.py:268 | #2 | Stores Home Assistant's `HVACMode` into a field the `hvac_mode` property (L133-140) compares against `WyzeHVACMode`: after any mode set, every branch misses and the property reports OFF. Repeated at L273, L278, L283. | `self._thermostat.hvac_mode = HVACMode.OFF` |
| P1-90 | custom_components/wyzeapi/climate.py:332 | #1 | Bare `dict` return annotation on a public property; `supported_features` (L324) is likewise annotated `int` for a `ClimateEntityFeature` flag. | `def device_info(self) -> dict:` |
| P1-91 | custom_components/wyzeapi/climate.py:118 | none | Two contradictory homes for one fact: this property hardcodes Fahrenheit with the real check commented out, while `unit_of_measurement` (L125-128) performs that check. | `# if self._thermostat.temp_unit == TemperatureUnit.FAHRENHEIT:` / `return UnitOfTemperature.FAHRENHEIT` |
| P1-92 | custom_components/wyzeapi/climate.py:241 | #11 | `async_set_fan_mode`, `async_set_hvac_mode` (L262) and `async_set_preset_mode` (L296) share one shape: an if/elif constant map, then an identical three-clause try/except/else setting `_server_out_of_sync` and scheduling an update. | `except (AccessTokenError, ParameterError, UnknownApiError) as err:` / `self._server_out_of_sync = True` |
| P1-93 | custom_components/wyzeapi/climate.py:131 | #11 | `hvac_mode` and `hvac_action` (L194-203) are the same four-branch enum translation; `preset_mode` (L156-164) is a third instance written as a `match`. Three spellings of one lookup table. | `if self._thermostat.hvac_mode == WyzeHVACMode.AUTO:` |
| P1-94 | custom_components/wyzeapi/climate.py:206 | #1 | Required arguments are pulled out of an opaque `**kwargs` bag by string key, with no signature stating that `target_temp_low`/`target_temp_high` are mandatory (the sync twin at L86 is the same). | `target_temp_low = kwargs["target_temp_low"]` |
| P1-95 | custom_components/wyzeapi/climate.py:1 | #29 | The climate module's docstring is the light module's, copied; lock.py:3 has the same wrong docstring, and this file's property docstrings say "this lock" (L352) and "this light" (L361). | `"""Platform for light integration."""` |
| P1-96 | custom_components/wyzeapi/lock.py:263 | #1 | `@property` methods declaring `**kwargs`, which a property can never receive (also L267). | `def is_locking(self, **kwargs):` |
| P1-97 | custom_components/wyzeapi/lock.py:264 | #30 | Reaches through the coordinator into private state: `_current_command` here, `_mac` (L241), `_uuid` (L243), `coordinator._lock` (L222). The coordinator publishes `data` but not these. | `return self.coordinator._current_command == "lock"` |
| P1-98 | custom_components/wyzeapi/lock.py:179 | none | Returns `None` where Home Assistant expects a `LockEntityFeature` int flag; any bitwise test against it raises. | `def supported_features(self):` / `return None` |
| P1-99 | custom_components/wyzeapi/lock.py:167 | #21 | The `raw_dict.get("power")` / `raw_dict.get("keypad", {}).get("power")` probes are re-derived here and again at sensor.py:155, 158, 227 and 229, each site calling `.get` twice to read one value. | `if self._lock.raw_dict.get("power"):` / `dev_info["lock_battery"] = str(self._lock.raw_dict.get("power"))` |
| P1-100 | custom_components/wyzeapi/lock.py:83 | #12 | List-membership syntax for a single-value comparison (same at cover.py:66). | `if self._lock.type not in [DeviceTypes.LOCK]:` |
| P1-101 | custom_components/wyzeapi/lock.py:105 | none | Sync stubs raising `NotImplementedError` beside their working async twins; the same dead-stub pattern is repeated eleven more times at climate.py:86-108 and alarm_control_panel.py:78-88. | `def lock(self, **kwargs):` / `raise NotImplementedError` |
| P1-102 | custom_components/wyzeapi/binary_sensor.py:208 | none | The decorator returns an `async def`, so a *sync* callback becomes a coroutine function; it is registered as a plain callback at L202, so each camera motion update produces a never-awaited coroutine and is dropped. `WyzeSensor.process_update` (L82) is left undecorated. | `@token_exception_handler` / `def process_update(self, camera: Camera) -> None:` |
| P1-103 | custom_components/wyzeapi/binary_sensor.py:71 | #12 | Round-trips through a string and appends three zero characters to mean `int(time.time()) * 1000`. | `self._last_event = int(str(int(time.time())) + "000")` |
| P1-104 | custom_components/wyzeapi/binary_sensor.py:150 | #9 | Class-level attribute evaluated once at *import* time: every `WyzeCameraMotion` created later starts its event clock from the module's import timestamp, not its own. | `_last_event = time.time() * 1000` |
| P1-105 | custom_components/wyzeapi/binary_sensor.py:217 | none | Both branches end with the same assignment; only `_is_on` differs, so the whole block is `self._is_on = camera.last_event_ts > self._last_event` followed by one write. | `self._is_on = True` / `self._last_event = camera.last_event_ts` |
| P1-106 | custom_components/wyzeapi/binary_sensor.py:102 | none | Hardcoded availability: the sensor reports available even when the sensor service is down, unlike every sibling entity which reads `device.available`. | `def available(self) -> bool:` / `return True` |
| P1-107 | custom_components/wyzeapi/binary_sensor.py:71 | none | `WyzeSensor._last_event` is assigned and never read; only the same-named field on `WyzeCameraMotion` is used. | `self._last_event = int(str(int(time.time())) + "000")` |
| P1-108 | custom_components/wyzeapi/binary_sensor.py:120 | none | `WyzeSensor` and `WyzeCameraMotion` (L185) mint the identical `{mac}-motion` unique-id shape, so a camera and a sensor sharing a MAC prefix collide; both are also the package's only `.format` calls. | `return "{}-motion".format(self._sensor.mac)` |
| P1-109 | custom_components/wyzeapi/cover.py:70 | none | `_available` is set in `__init__` and never read; `available` (L125) reads the device. | `self._available = self._camera.available` |
| P1-110 | custom_components/wyzeapi/cover.py:50 | none | Unguarded index on an optional key for *every* camera during setup — one camera without a dongle raises `KeyError` and the cover platform never loads. | `if camera.device_params["dongle_product_model"] == "HL_CGDC":` |
| P1-111 | custom_components/wyzeapi/cover.py:56 | none | `ABC` is mixed into a fully concrete entity (also lock.py:77), declaring an abstractness the class does not have. | `class WyzeGarageDoor(homeassistant.components.cover.CoverEntity, ABC):` |
| P1-112 | custom_components/wyzeapi/cover.py:94 | #1 | Untyped opaque `**kwargs` and no return annotation on both public command methods (also L107). | `async def async_open_cover(self, **kwargs):` |
| P1-113 | custom_components/wyzeapi/number.py:69 | #8 | The `{mac}-zone-{n}-quickrun-duration` unique id is a cross-module contract with four homes: constructed here, independently reconstructed at button.py:164, and restated in prose at button.py:140 and 162. | `return f"{self._device.mac}-zone-{self._zone.zone_number}-quickrun-duration"` |
| P1-114 | custom_components/wyzeapi/number.py:104 | #8 | Returns the raw string `"box"` where `NumberMode.BOX` exists; button.py:114 and 259 likewise annotate `device_class -> str` for a `ButtonDeviceClass`. | `def mode(self) -> str:` / `return "box"` |
| P1-115 | custom_components/wyzeapi/number.py:137 | none | Silently discards a corrupt restored value with no log, then falls through to a network call — the user sees a duration reset with no explanation. | `except (ValueError, TypeError):` / `pass` |
| P1-116 | custom_components/wyzeapi/button.py:168 | #12 | Linear scan over the entire Home Assistant entity registry to find one entity by unique id, on every button press; `er.async_get_entity_id(...)` does exactly this lookup and sensor.py:506 already uses it. | `for entity_id, entity in entity_registry.entities.items():` |
| P1-117 | custom_components/wyzeapi/button.py:150 | #18 | `async_press` is a 90-line method narrated in five labelled phases: L150, L158, L161, L195, L214. | `# Get the device registry and find the device` |
| P1-118 | custom_components/wyzeapi/button.py:139 | #7 | The docstring states the caller-side coupling ("found using the exact unique_id pattern that follows the format …") for a contract nothing enforces; it silently breaks if number.py:69 changes. | `The number entity is found using the exact unique_id pattern that follows the format:` |
| P1-119 | custom_components/wyzeapi/button.py:114 | #1 | Public property annotated `str` while returning a `ButtonDeviceClass` (also L259); `RESTART` is additionally the wrong class for "start this irrigation zone". | `def device_class(self) -> str:` / `return ButtonDeviceClass.RESTART` |
| P1-120 | custom_components/wyzeapi/button.py:299 | #11 | Three `device_info` properties in one file with three different field sets for the same vendor's devices: this one omits manufacturer and model, L247 adds `serial_number`, L103 omits it. | `return DeviceInfo(identifiers={(DOMAIN, self._switch.mac)}, name=self._switch.nickname,)` |
| P1-121 | custom_components/wyzeapi/alarm_control_panel.py:101 | #8 | Writes raw strings into a field that `__init__` (L70) and `async_update` (L156-162) fill with `AlarmControlPanelState` members; also L113 and L125. One state, two representations. | `self._state = "disarmed"` |
| P1-122 | custom_components/wyzeapi/alarm_control_panel.py:74 | #1 | Public property annotated `str` while returning `AlarmControlPanelState`; `supported_features` (L129) annotates a feature flag as `int`. | `def alarm_state(self) -> str:` |
| P1-123 | custom_components/wyzeapi/alarm_control_panel.py:91 | #11 | `async_alarm_disarm` (L92), `async_alarm_arm_home` (L105) and `async_alarm_arm_away` (L117) are the same eleven-line body with one enum member and one string literal changed. | `await self._hms_service.set_mode(HMSMode.DISARMED)` |
| P1-124 | custom_components/wyzeapi/alarm_control_panel.py:62 | none | `AVAILABLE = True` is declared and never read. | `AVAILABLE = True` |
| P1-125 | custom_components/wyzeapi/alarm_control_panel.py:77 | #18 | Two labelled phases of the class body carried in prose comments (also L90) — the split they describe wants to be structure. | `# NotImplemented Methods` |
| P1-126 | custom_components/wyzeapi/alarm_control_panel.py:164 | none | Eager f-string in a log call (also coordinator.py:178-182) against the package's own lazy-`%s` convention; formats even when the level is disabled. | `_LOGGER.warning(f"Received {state} from server")` |
| P1-127 | custom_components/wyzeapi/ydble_utils.py:4 | none | `pycryptodome` is imported by prod code but declared in neither `pyproject.toml` `dependencies` (homeassistant, wyzeapy, websockets) nor `manifest.json` `requirements`; the lock platform's BLE path depends on an undeclared package. | `from Crypto.Cipher import AES` |

## Phase 2 — audit finding verdicts

117 findings. Deps were unavailable, so oracle rules (#2 and #4's proof step)
under-fired (0 proved) — every finding here is heuristic/indexed. Grouped where a
rule fires near-identically; exceptions split out.

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| #11 clones x20: light.py:463, siren.py:135/121, switch.py:552/638/488/574/658, sensor.py:334/469/537/254/708/727, button.py:247/84, number.py:72/53, camera.py:136, cover.py:139 | #11 | indexed | real | Every group is a genuine AST-identical body — the ~20x `device_info` dict, the dispatcher `async_added_to_hass`, the service+device `__init__` — repeated across sibling entity classes; first-copy exemption still leaves >=1 extra copy each. |
| #1 weak-boundary x36: the 13 `async_add_entities: Callable[[list[Any],bool],None]` setup params; the `**kwargs` command/property methods (climate.py:206/86, cover.py:94/107, light.py:396/410, lock.py:116/129/105/108/256/259/263/267, siren.py:71/85); the `dict[str,Any]`/bare-dict returns (config_flow.py:60/103/104, climate.py:332, button.py:124, sensor.py:679/741) | #1 | heuristic | real | Each is a genuinely weak published contract: `list[Any]` where HA ships `AddConfigEntryEntitiesCallback`, opaque `**kwargs` that hide the real accepted keys (light's actually reads `ATTR_BRIGHTNESS` etc.), and `dict`/`Any` returns where `DeviceInfo`/`Mapping` exist. |
| custom_components/wyzeapi/token_manager.py:42 | #1 | heuristic | fp | `inner_function(*args, **kwargs)` is a transparent decorator forwarder — `**kwargs` is correct and necessary; the real defect (dropped return / no `functools.wraps`) is outside #1's lens. |
| #26 computed-declaration x7: alarm_control_panel.py:29, climate.py:44, light.py:46, lock.py:30, switch.py:50 (`SCAN_INTERVAL = timedelta(...)`); config_flow.py:28/36 (`vol.Schema({literal})`) | #26 | heuristic | fp | `timedelta(seconds=15)` and `vol.Schema({literal dict})` are transparent literal constants with no members to enumerate — the reader executes nothing to know them. The rule's ideal (dynamic `__all__`/star-import/assembled list) does not apply; the one genuine computed declaration (the CRC table, P1-20) went unflagged. |
| #30 demeter x7: light.py:296, sensor.py:155/227/301, switch.py:400, lock.py:167/252 | #30 | heuristic | real | Each reaches through an opaque data bag — `.raw_dict.get(...)`, `.device_params.get(...)`, `.coordinator.data.get(...)` — structure hidden from the signature, exactly the reach the ideal names. |
| #30 demeter x10: light.py:149/252/362, switch.py:452/319, button.py:189, __init__.py:83, config_flow.py:90/120/168 | #30 | heuristic | fp | Idiomatic framework navigation (`self.hass.config_entries.async_entries`, `hass.bus.fire`, `hass.states.get`, `config_entry.options.get`) or a method on a retrieved string (`self._device.mac.split`) — documented, honest coupling, not a Demeter smell. |
| #21 invariant x3: light.py:99 (`options.get(BULB_LOCAL_CONTROL)`), fan.py:65 (`self._air_purifier_service.turn_on(...)`), camera.py:270 (`self.websocket is None`) | #21 | heuristic | real | Each is a genuine domain expression/guard repeated across >=3 methods that a named accessor or guard could own — the options re-derivation is P1-75. |
| #21 invariant x9: climate.py:74, fan.py:65, coordinator.py:31, cover.py:56, light.py:99, lock.py:77, switch.py:292/477/563 (`async_schedule_update_ha_state` / `async_write_ha_state` / `async_update_listeners`) | #21 | heuristic | fp | The mandated HA "mutate then notify" side-effect calls every entity method must issue — not an invariant that belongs in the type (the CaseInsensitiveMap ideal); nothing to encapsulate. |
| #15 wallet x8: switch.py:67, __init__.py:52 (framework `config_entry`/`hass`); coordinator.py:138 (forwarded `client`), coordinator.py:121 (`seq_no: int`); tests/test_air_purifier_fan.py:119x2/137/179 | #15 | mixed | fp | Framework entry points can't narrow their HA-fixed signature; `client` is forwarded and `BleakClient` is the honest type; `seq_no` is a plain `int` whose only "demand" is `.to_bytes`; the rest are test mocks. None is a genuine wallet. |
| custom_components/wyzeapi/alarm_control_panel.py:33 | #14 | indexed | fp | `(async_add_entities, config_entry, hass)` recur because HA fixes the `async_setup_entry` signature — they cannot be replaced by a type. |
| tests/test_air_purifier_fan.py:119 | #14 | indexed | fp | `(air_purifier, entity, service)` recur across test functions as fixtures; "wants a type" is not a real improvement to test code. |
| custom_components/wyzeapi/camera.py:212 | #8 | indexed | real | `session_id: str` threads through 4 WebRTC methods as a dict key; a `NewType` would stop it mixing with other strings — genuine primitive obsession. |
| custom_components/wyzeapi/config_flow.py:54 | #6 | indexed | real | `get_client` is accessor-named, returns nothing, exists only to mutate `self.client` — a getter that lies (= P1-30). |
| custom_components/wyzeapi/coordinator.py:199 | #5 | indexed | real | `delay` in `_disconnect(self, delay=0)` is unannotated; `delay: int` is correct at both call sites and verified counterfactually — a valid lift proposal. |
| custom_components/wyzeapi/ydble_utils.py:90 | #5 | indexed | real | `crc(data)` is unannotated; `data: bytes` holds at all 3 call sites (iterated as bytes) — valid lift proposal. |
| custom_components/wyzeapi/sensor.py:765 | #4 | indexed | fp | The `timestamp is None` guard is NOT established by callers: `extra_state_attributes` (L746/749/753) passes `max_hourly_aqi_start/end_time` — genuinely `int|None` — with no prior None-check; removing the guard crashes on None. The WP claim is unsound (incomplete graph, deps missing). |
| custom_components/wyzeapi/ydble_utils.py:47 | #10 | indexed | real | `pack_l2_dict` demands `Dict[int,bytes]` but the body only does `.items()` — `Mapping[int,bytes]` suffices; widening verified. |
| README.md:76 | #28 | indexed | fp | `configuration.yaml` is the Home Assistant user's own config file named in prose ("add this to your `configuration.yaml`"), not a repo path — nothing to resolve. |
| #29 top-loading x3: coordinator.py:1, ydble_utils.py:1, config_flow.py:102 | #29 | heuristic | real | coordinator (204 L) and ydble_utils (116 L, 8 defs) have no module docstring (= P1-12); `async_step_2fa` is a 30-line network entry point with no cost docstring. |
| #17 liveness-neck x2: camera.py:92, camera.py:289 | #17 | heuristic | fp | Both necks are inside `__init__` (1 crossing) — sequential attribute initialisation is not a genuine split point; #17 is report-only and these are noise. |
| #22 velcro x2: config_flow.py:134, camera.py:385 | #22 | heuristic | fp | `async_step_import` is a framework-dispatched config-flow lifecycle method (HA calls it by name); `close_connection` operates on `self.close` instance state — neither is a free function hiding in a class. |

## Phase 3 — reconciliation

Classes: covered 13, detector-miss 64, threshold-miss 8, inventory-gap 42.
The high detector-miss count reflects the environment: oracle rule #2 fired
nowhere (deps unavailable), and #7/#12/#18/#19/#9/#3 fired nowhere at all this run.

| P1 id | rule | class | note |
|-------|------|-------|------|
| P1-1 | #2 | detector-miss | Unreachable branch; #2 is oracle and fired 0x (deps missing). |
| P1-2 | #1 | detector-miss | `discovery_info=None` is implicitly untyped, not explicit `Any` — below #1's trigger. |
| P1-3 | none | inventory-gap | Stores constant names not values — no rule covers value/name confusion. |
| P1-4 | none | inventory-gap | Unregistered listener (dead) — no dead-code rule in the inventory. |
| P1-5 | #4 | detector-miss | Dead `setdefault` default; #4 fired only once and not here. |
| P1-6 | #8 | detector-miss | Repeated model literal `"YD_BT1"` is not a `*_id: str` param — outside #8's shape. |
| P1-7 | #1 | detector-miss | Untyped `hass.data` bag is an inline dict, not a signature — #1 is signature-scoped. |
| P1-8 | #9 | detector-miss | Class-attr-as-global; #9 fired 0x. |
| P1-9 | #3 | detector-miss | Emptiness guard; #3 fired 0x. |
| P1-10 | #1 | detector-miss | `Token = None` non-optional default is not explicit `Any`. |
| P1-11 | none | inventory-gap | Decorator drops return / no `wraps` — no rule covers decorator fidelity. |
| P1-12 | #29 | covered | #29 fired on coordinator.py:1 and ydble_utils.py:1 (token_manager is below the size threshold). |
| P1-13 | #1 | threshold-miss | `context: Dict` is on the private `_handle_uart_rx`; #1 is scoped to public boundaries. |
| P1-14 | #18 | detector-miss | Multi-phase narration; #18 fired 0x. |
| P1-15 | #11 | threshold-miss | The three send-helper bodies are 2-3 lines — under the clone size cutoff. |
| P1-16 | #11 | threshold-miss | The `_get_ble_client` None-check dup is a sub-function block, under the clone cutoff. |
| P1-17 | none | inventory-gap | Bare `Exception` on a user path — no exception-hygiene rule. |
| P1-18 | none | inventory-gap | Fire-and-forget task — no rule covers orphaned tasks. |
| P1-19 | #1 | detector-miss | `command` implicitly untyped, not explicit `Any`. |
| P1-20 | #26 | detector-miss | The genuine computed CRC table lives inside `crc()`; #26 only scanned module-level and fired on trivial constants instead. |
| P1-21 | none | inventory-gap | Table rebuilt per call (perf) — no rule covers loop-invariant recompute of a constant. |
| P1-22 | none | inventory-gap | `crc()` computed twice — no rule covers redundant recomputation. |
| P1-23 | #1 | detector-miss | Missing return annotation is not `Any`; #1 needs an explicit weak type. |
| P1-24 | #11 | threshold-miss | `encrypt_ecb`/`decrypt_ecb` are 4-line twins — under the clone cutoff (or first-copy exempt). |
| P1-25 | #1 | detector-miss | `command` implicitly untyped, not explicit `Any`. |
| P1-26 | none | inventory-gap | Unnamed hex protocol frames — no rule covers magic-number naming. |
| P1-27 | #1 | covered | #1 fired on this signature's return at config_flow.py:60. |
| P1-28 | #9 | detector-miss | Class-level mutable `user_params`; #9 fired 0x. |
| P1-29 | #1 | detector-miss | `client = None` non-optional is not explicit `Any`. |
| P1-30 | #6 | covered | Finding config_flow.py:54 #6. |
| P1-31 | none | inventory-gap | Unreachable `except` (dead exception class) — no dead-code rule. |
| P1-32 | none | inventory-gap | Unused `InvalidAuth` — no dead-code rule. |
| P1-33 | none | inventory-gap | Dead instance fields — no dead-code rule. |
| P1-34 | #3 | detector-miss | Emptiness guard; #3 fired 0x (though #30 fired at the same line for a different reach). |
| P1-35 | #11 | threshold-miss | The reauth-vs-create block dup is intra-function, under clone granularity. |
| P1-36 | #13 | covered | A finding fired at config_flow.py:134 — as #22 velcro rather than #13, but the site is caught. |
| P1-37 | #15 | detector-miss | `async_get_options_flow` never uses `config_entry`; wallet fired elsewhere, not here. |
| P1-38 | #28 | detector-miss | 2FA schema/`strings.json` key mismatch is translation-key integrity; #28 as implemented checks doc paths. |
| P1-39 | #28 | detector-miss | `keyid`/`apikey` vs `key_id`/`api_key` — same translation-key gap, not a doc path. |
| P1-40 | #28 | detector-miss | `reauth_confirm` step absent from `strings.json` — same class as P1-38/39. |
| P1-41 | #1 | covered | Finding siren.py:71 #1 opaque `**kwargs`. |
| P1-42 | none | inventory-gap | Dead `_available` — no dead-code rule. |
| P1-43 | none | inventory-gap | Dead `_just_updated` — no dead-code rule. |
| P1-44 | #11 | covered | Finding siren.py:121 #11 device_info clone group. |
| P1-45 | #8 | detector-miss | Model-list predicate at >=8 sites is not a `*_id` param; #8 caught only `session_id`. |
| P1-46 | #11 | covered | The duplicated methods of both classes fired as clones (switch.py:488/552/574/638). |
| P1-47 | #2 | detector-miss | `isinstance` unreachable under annotation; #2 oracle fired 0x. |
| P1-48 | none | inventory-gap | `mac` leaks from loop (NameError risk) — no rule covers loop-var escape. |
| P1-49 | #11 | detector-miss | ip/rssi/ssid block is intra-function; #11 works at function granularity. |
| P1-50 | none | inventory-gap | `str(x + "%")` TypeError — no rule covers type-confused concatenation. |
| P1-51 | #19 | detector-miss | Membership in setup loop; #19 fired 0x. |
| P1-52 | #12 | detector-miss | One-element list for `==`; #12 fired 0x. |
| P1-53 | #15 | detector-miss | `async_migrate_switch_data` wallet/untyped params; wallet fired at switch.py:67 not here. |
| P1-54 | #11 | detector-miss | Device-issue block dup is intra-function. |
| P1-55 | none | inventory-gap | `Wzye` typo — no rule covers identifier spelling. |
| P1-56 | none | inventory-gap | Wrong log string — no rule covers log-message accuracy. |
| P1-57 | none | inventory-gap | `OUTDOOR_PLUGS` str-vs-list drift — no single-source-of-truth rule for constants. |
| P1-58 | #12 | detector-miss | List rebuild in loop; #12 fired 0x. |
| P1-59 | #1 | detector-miss | `now: datetime` (wrong module) + unused — a wrong type, not `Any`. |
| P1-60 | #2 | detector-miss | Dead `is False` guard; #2 oracle fired 0x. |
| P1-61 | none | inventory-gap | Property shadows `Entity.enabled` — no rule covers base-class override hazards. |
| P1-62 | #21 | detector-miss | `_battery_type ==` discriminator differs per branch; #21 needs one identical expression >=3x. |
| P1-63 | #1 | detector-miss | Untyped `__init__` params, not explicit `Any`. |
| P1-64 | #6 | detector-miss | Foreign-arg mutation in `__init__`; #6 caught only `get_client`. |
| P1-65 | #6 | detector-miss | `update_energy` mutate-and-return; #6 did not fire here. |
| P1-66 | #18 | detector-miss | Phase comments; #18 fired 0x. |
| P1-67 | #11 | threshold-miss | RSSI/IP/SSID bodies are tiny property methods — under the clone size cutoff. |
| P1-68 | #11 | detector-miss | sensor/fan device_info are typed-`DeviceInfo`-vs-dict — not AST-identical, so no clone. |
| P1-69 | #19 | detector-miss | Membership in comprehension; #19 fired 0x. |
| P1-70 | #1 | detector-miss | Untyped `native_value` (str/int mix), not explicit `Any` (#30 fired at 227 for the reach). |
| P1-71 | #12 | detector-miss | `any([single])`; #12 fired 0x. |
| P1-72 | #17 | detector-miss | 100-line method; `options` stays live throughout so no neck surfaced; #17 fired only on `__init__`s. |
| P1-73 | #21 | detector-miss | Effect map forward/backward is two different expressions; #21 needs one repeated. |
| P1-74 | none | inventory-gap | `supported_color_modes` scalar-not-set — no rule covers return-shape contracts. |
| P1-75 | #21 | covered | Finding light.py:99 #21 `options.get(BULB_LOCAL_CONTROL)` in 3 methods. |
| P1-76 | none | inventory-gap | Dead `_is_on` — no dead-code rule. |
| P1-77 | #1 | detector-miss | Untyped `config_entry` param, not explicit `Any`. |
| P1-78 | none | inventory-gap | KeyError on optional dict key in setup — no rule covers unguarded index. |
| P1-79 | #7 | detector-miss | Comment narrating dead `_config_task`; #7 fired 0x. |
| P1-80 | #7 | detector-miss | Precondition in comment; #7 fired 0x. |
| P1-81 | none | inventory-gap | Assigns over `CameraEntity` properties — no rule (an #17 finding sits at L92 for an unrelated neck). |
| P1-82 | none | inventory-gap | `@cached_property` on live state — no rule covers cache-staleness. |
| P1-83 | none | inventory-gap | Dead `camera_service` — no dead-code rule. |
| P1-84 | none | inventory-gap | Hardcoded client-id literal x2 — no magic-literal rule. |
| P1-85 | #11 | detector-miss | `send_candidate`/`send_offer` are similar but not AST-identical — no clone. |
| P1-86 | #19 | detector-miss | Quadratic `finditer` in loop; #19 fired 0x. |
| P1-87 | #1 | detector-miss | `_cached_config: dict` is an attribute annotation, not a public signature. |
| P1-88 | none | inventory-gap | `getattr(x, "motion")` uses a literal name — outside #24 (constructed names); correctly `none`. |
| P1-89 | #2 | detector-miss | HA/Wyze enum type confusion; #2 oracle fired 0x. |
| P1-90 | #1 | covered | Finding climate.py:332 #1 bare dict return. |
| P1-91 | none | inventory-gap | Contradictory temp-unit homes — no rule covers dead-commented logic. |
| P1-92 | #11 | detector-miss | The 3 set-methods share shape but differ in enum bodies — not AST-identical. |
| P1-93 | #11 | detector-miss | hvac_mode/action/preset are similar lookups but distinct expressions — no clone. |
| P1-94 | #1 | covered | Finding climate.py:206 #1 opaque `**kwargs`. |
| P1-95 | #29 | detector-miss | Copied wrong docstring — #29 checks presence, not correctness; docstring is present. |
| P1-96 | #1 | covered | Findings lock.py:263/267 #1 opaque `**kwargs`. |
| P1-97 | #30 | threshold-miss | #30 fired in the same `WyzeLockBolt` class (lock.py:252) but not on the adjacent `_current_command` reach. |
| P1-98 | none | inventory-gap | `supported_features` returns None — no rule covers return-contract violations. |
| P1-99 | #21 | covered | Finding lock.py:167 #30 caught the same `raw_dict.get` reach (I mapped #21; the site is flagged). |
| P1-100 | #12 | detector-miss | List-membership for single `==`; #12 fired 0x. |
| P1-101 | none | inventory-gap | Sync `NotImplementedError` stubs — no dead-stub rule (#1 fired at L105/108 for `**kwargs`, a different claim). |
| P1-102 | none | inventory-gap | Decorator turns sync callback into dropped coroutine — no rule covers async/callback contracts. |
| P1-103 | #12 | detector-miss | String round-trip for `*1000`; #12 fired 0x. |
| P1-104 | #9 | detector-miss | `time.time()` class attr at import; #9 fired 0x. |
| P1-105 | none | inventory-gap | Both branches share assignment — no rule covers branch redundancy. |
| P1-106 | none | inventory-gap | Hardcoded `available = True` — no rule covers availability honesty. |
| P1-107 | none | inventory-gap | Dead `_last_event` on WyzeSensor — no dead-code rule. |
| P1-108 | none | inventory-gap | unique_id collision — no rule covers id uniqueness. |
| P1-109 | none | inventory-gap | Dead `_available` — no dead-code rule. |
| P1-110 | none | inventory-gap | KeyError in setup — no unguarded-index rule. |
| P1-111 | none | inventory-gap | `ABC` on concrete class — no rule covers spurious abstractness. |
| P1-112 | #1 | covered | Findings cover.py:94/107 #1 opaque `**kwargs`. |
| P1-113 | #8 | detector-miss | Cross-module unique_id contract is a constructed literal, not a `*_id` param; #8 missed it. |
| P1-114 | #8 | detector-miss | `"box"` / `device_class -> str` stringly-typed; #8 caught only `session_id`. |
| P1-115 | none | inventory-gap | Silent `except: pass` — no rule covers swallowed errors. |
| P1-116 | #12 | detector-miss | Reinvented `async_get_entity_id` via linear scan; #12 fired 0x. |
| P1-117 | #18 | detector-miss | 90-line 5-phase method; #18 fired 0x. |
| P1-118 | #7 | detector-miss | Docstring states caller coupling; #7 fired 0x. |
| P1-119 | #1 | detector-miss | `device_class -> str` returns an enum — a wrong type, not `Any`. |
| P1-120 | #11 | threshold-miss | The three `device_info` variants: the base pair fired (button.py:247), the divergent reset-button one (L299) did not. |
| P1-121 | #8 | detector-miss | Raw string state vs enum — not a `*_id` param; #8 missed it. |
| P1-122 | #1 | detector-miss | `alarm_state -> str` returns an enum — wrong type, not `Any`. |
| P1-123 | #11 | detector-miss | The 3 arm-methods differ in one enum/string — not AST-identical, so no clone. |
| P1-124 | none | inventory-gap | Dead `AVAILABLE` — no dead-code rule. |
| P1-125 | #18 | detector-miss | Section comments; #18 fired 0x. |
| P1-126 | none | inventory-gap | Eager f-string log — no rule covers lazy-logging convention. |
| P1-127 | none | inventory-gap | Undeclared `pycryptodome` dependency — no rule covers dependency-manifest integrity. |
