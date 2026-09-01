# cityscapesScripts — wave 1

| rule | fp class | count | example key |
| --- | --- | --- | --- |
| 5 | param already declared by a PEP 484 type comment; the rule reads only inline annotations | 13 | cityscapesscripts/evaluation/evalObjectDetection3d.py:512:5:lift:cityscapesscripts.evaluation.evalObjectDetection3d.Box3dEvaluator._getMatches:matchIgnores |
| 11 | literal-blind windows over a run of `message += "literal"` help-text statements (self-matching inside one function) | 21 | cityscapesscripts/annotation/cityscapesLabelTool.py:1148:11:clone-block:9fad884aeb9a |
| 11 | literal-blind windows over a run of `print('literal')` help-text statements | 17 | cityscapesscripts/preparation/json2instanceImg.py:49:11:clone-block:daf23c3d9a9a |
| 11 | overlapping windows not deduped: the same file:line already reported by a wider window | 2 | cityscapesscripts/annotation/cityscapesLabelTool.py:533:11:clone-block:52b7b53b7a9e |
| 11 | one-statement generic `json.dumps` body counted as a whole-function clone | 2 | cityscapesscripts/evaluation/instance.py:29:11:clone:0de3cc835627 |
| 23 | flat breadth at nesting depth 0-1 (key dispatch, linear paint sequence) scored at the threshold | 2 | cityscapesscripts/annotation/cityscapesLabelTool.py:2058:23:cognitive-complexity:cityscapesscripts.annotation.cityscapesLabelTool.CityscapesLabelTool.keyPressEvent |
| 27 | a cohesive ~400-line helper module whose "hot symbols" are the classes it exists to define | 2 | cityscapesscripts/helpers/annotation.py:1:27:price:cityscapesscripts.helpers.annotation |
| 29 | the first screen already is the map: a multi-line `#` header block instead of a docstring | 10 | cityscapesscripts/evaluation/evalInstanceLevelSemanticLabeling.py:1:29:top-loading:cityscapesscripts.evaluation.evalInstanceLevelSemanticLabeling |
| 29 | the map is a triple-quoted block that sits after the imports, so it is not the module docstring | 1 | cityscapesscripts/helpers/box3dImageTransform.py:1:29:top-loading:cityscapesscripts.helpers.box3dImageTransform |
| 32 | import used only by the file's PEP 484 type comments | 6 | cityscapesscripts/evaluation/evalObjectDetection3d.py:42:32:dead-import:cityscapesscripts.evaluation.evalObjectDetection3d:List |
| 32 | import re-exported through `from csHelpers import *` and used by the star importers | 6 | cityscapesscripts/helpers/csHelpers.py:23:32:dead-import:cityscapesscripts.helpers.csHelpers:np |
| 32 | public API exercised only by a shipped notebook (docs/Box3DImageTransform.ipynb) | 3 | cityscapesscripts/helpers/box3dImageTransform.py:159:32:dead-symbol:cityscapesscripts.helpers.box3dImageTransform.Box3dImageTransform.get_vertices |
| 32 | one constant of a complete ANSI palette class, where the set is the unit | 2 | cityscapesscripts/helpers/csHelpers.py:42:32:dead-symbol:cityscapesscripts.helpers.csHelpers.colors.MAGENTA |
| 32 | published lookup table twinned with one the repo does import, in a data-definition module | 1 | cityscapesscripts/helpers/labels_cityPersons.py:60:32:dead-symbol:cityscapesscripts.helpers.labels_cityPersons.id2labelCp |
| 39 | section header over a multi-import block read as a per-line restatement | 1 | cityscapesscripts/viewer/cityscapesViewer.py:34:39:comment-restates:cityscapesscripts.viewer.cityscapesViewer:34 |
| 50 | every named slot already carries a PEP 484 type comment | 21 | cityscapesscripts/evaluation/evalObjectDetection3d.py:161:50:unannotated:cityscapesscripts.evaluation.evalObjectDetection3d.Box3dEvaluator.loadGT |
| 50 | override of a PyQt5 QWidget/QMainWindow handler; the signature is the framework's contract | 14 | cityscapesscripts/annotation/cityscapesLabelTool.py:1190:50:unannotated:cityscapesscripts.annotation.cityscapesLabelTool.CityscapesLabelTool.closeEvent |
| 55 | one required param plus six documented defaults the only call site already passes by keyword | 1 | cityscapesscripts/evaluation/objectDetectionHelpers.py:29:55:positional-width:cityscapesscripts.evaluation.objectDetectionHelpers.EvaluationParameters.__init__ |
| 59 | Qt slot / GUI dialog method, not a callable entry point | 6 | cityscapesscripts/annotation/cityscapesLabelTool.py:647:59:cost-docstring:cityscapesscripts.annotation.cityscapesLabelTool.CityscapesLabelTool.selectCity |
| 59 | main-by-another-name under a `__name__` guard, documented by the module's header block | 5 | cityscapesscripts/evaluation/evalInstanceLevelSemanticLabeling.py:680:59:cost-docstring:cityscapesscripts.evaluation.evalInstanceLevelSemanticLabeling.main |
| 59 | the spend is evident from the def's own name and parameters | 3 | cityscapesscripts/evaluation/instances2dict.py:13:59:cost-docstring:cityscapesscripts.evaluation.instances2dict.instances2dict |
| 59 | the cost is already declared by the `#` comment directly above the def | 2 | cityscapesscripts/annotation/cityscapesLabelTool.py:1305:59:cost-docstring:cityscapesscripts.annotation.cityscapesLabelTool.CityscapesLabelTool.loadCorrections |
