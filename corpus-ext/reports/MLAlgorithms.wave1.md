# MLAlgorithms — wave 1

## Phase 1 — blind ideal sites

| id | site (path:line) | rule | claim | excerpt (<=3 lines) |
|----|------------------|------|-------|---------------------|
| P1-1 | mla/kmeans.py:95 | #19 | `_assign` scans every cluster list for `row` membership then `.remove`, inside a loop over all samples: O(n_samples · K · cluster) list ops per iteration | `if row in cluster:` / `    self.clusters[i].remove(row)` |
| P1-2 | mla/rl/dqn.py:131 | #19 | replay buffer trimmed with `list.pop(0)` in a loop — each pop is O(n); a deque would be O(1) | `while len(self.replay) > self.memory_limit:` / `    self.replay.pop(0)` |
| P1-3 | mla/ensemble/base.py:41 | #11 | `get_split_mask` and `split` (l.47) duplicate the same `X < value` / `X >= value` mask pair | `left_mask = X[:, column] < value` / `right_mask = X[:, column] >= value` |
| P1-4 | mla/neuralnet/optimizers.py:83 | #11 | six `Optimizer.update` bodies (SGD/Adagrad/Adadelta/RMSprop/Adam/Adamax) repeat the same `for i,layer ... for n in keys(): grad=...grad[n]` scaffold | `for i, layer in enumerate(network.parametric_layers):` / `    for n in layer.parameters.keys():` |
| P1-5 | mla/neuralnet/layers/normalization.py:43 | #18 | `_forward_pass` narrates nine literal numbered phases (`# step1:`…`# step9`) inside one function | `# step1: calculate mean` / `mu = 1.0 / N * np.sum(X, axis=0)` |
| P1-6 | mla/neuralnet/layers/normalization.py:104 | #18 | `_backward_pass` narrates numbered phases `# step9`…`# step0` inside one function | `# step9` / `dbeta = np.sum(delta, axis=0)` |
| P1-7 | mla/metrics/metrics.py:88 | #24 | `get_metric` resolves functions by `globals()[name]` — a runtime-constructed lookup grep can't follow and that blinds the call graph | `return globals()[name]` |
| P1-8 | mla/neuralnet/activations.py:62 | #24 | `get_activation` dispatches via `globals()[name]` (same runtime name resolution) | `return globals()[name]` |
| P1-9 | mla/neuralnet/loss.py:9 | #24 | `get_loss` dispatches via `globals()[name]`; only catches `KeyError`, error text says "metric" | `return globals()[name]` |
| P1-10 | mla/neuralnet/initializations.py:73 | #24 | `get_initializer` dispatches via `globals()[name]` (same pattern, 4th copy in tree) | `return globals()[name]` |
| P1-11 | mla/neuralnet/layers/__init__.py:2 | #26 | three `from .X import *` star imports hide the package's exported surface from a reader/grep | `from .basic import *` / `from .convnet import *` / `from .normalization import *` |
| P1-12 | mla/metrics/__init__.py:2 | #26 | `from .metrics import *` star import re-exports an opaque surface | `from .metrics import *` |
| P1-13 | mla/base/__init__.py:2 | #26 | `from .base import *` star import re-exports an opaque surface | `from .base import *` |
| P1-14 | mla/neuralnet/initializations.py:18 | #1 | public initializer functions (`zero`, `one` l.22, and `**kwargs` on glorot/he) take opaque `**kwargs` that are silently discarded | `def zero(shape, **kwargs):` / `    return np.zeros(shape)` |
| P1-15 | mla/ensemble/tree.py:70 | #14 | `(X, target, max_features, min_samples_split, max_depth, minimum_gain)` travels together across `_train`, `train` (l.126) and the ensemble callers — an unnamed concept | `def _train(self, X, target, max_features=None, min_samples_split=10, max_depth=None, minimum_gain=0.01):` |
| P1-16 | mla/svm/svm.py:118 | #12 | `clip` hand-rolls a scalar clamp already provided by `np.clip` / `max(L, min(a, H))` | `def clip(self, alpha, H, L):` / `    if alpha > H: alpha = H` / `    if alpha < L: alpha = L` |
| P1-17 | mla/neuralnet/regularizers.py:14 | #13 | `grad` forwards to `self._grad(weights)` and `__call__` (l.17) forwards to `self.grad(weights)`: stacked forward-only wrappers | `def grad(self, weights):` / `    return self._grad(weights)` |
| P1-18 | mla/linear_models.py:11 | #9 | module-import `np.random.seed(1000)` mutates numpy's global RNG; repeated across ~9 modules (pca, fm, nnet, svm, rbm, tsne, dqn + `random.seed` in kmeans/tree) — action at a distance | `np.random.seed(1000)` |
| P1-19 | mla/metrics/base.py:12 | #2 | `type(a) != type(b)` is provably always False: both `a` and `b` were coerced to `np.ndarray` on lines 6-10, so the guard is dead | `if type(a) != type(b):` / `    raise ValueError(...)` |
| P1-20 | mla/fm.py:76 | none | `FMRegressor.fit` calls `super().fit()` (which runs `_train` using `self.loss_grad`) BEFORE assigning `loss`/`loss_grad`; grad is None during training (same in `FMClassifier` l.83) | `super(FMRegressor, self).fit(X, y)` / `self.loss_grad = elementwise_grad(mean_squared_error)` |
| P1-21 | mla/neuralnet/optimizers.py:91 | none | SGD nesterov branch recomputes `update` identically to line 87 (velocity already equals it), so the Nesterov correction is a no-op | `update = self.momentum * self.velocity[i][n] - lr * grad` |
| P1-22 | mla/metrics/metrics.py:69 | none | `hinge` passes `0.0` as the `axis` argument to `np.max`; intended element-wise floor needs `np.maximum(..., 0.0)` | `return np.mean(np.max(1.0 - actual * predicted, 0.0))` |
| P1-23 | mla/neuralnet/layers/recurrent/lstm.py:184 | none | `backward_pass` returns `output`, allocated as zeros and never written (`# TODO: propagate error`), so no gradient flows to prior layers | `# TODO: propagate error to the next layer` / `return output` |
| P1-24 | mla/datasets/base.py:12 | none | inner `load` default `dataset="training"` never matches its own guards (`"train"`/`"test"`) — the default value is dead and would raise | `def load(dataset="training", digits=np.arange(10)):` |
| P1-25 | mla/neuralnet/layers/convnet.py:210 | none | `convoltuion_shape` is misspelled and the misspelling is propagated to all call sites (l.72,123,157,189) — a consistent-but-wrong shared name | `def convoltuion_shape(img_height, img_width, filter_shape, stride, padding):` |
| P1-26 | mla/neuralnet/nnet.py:52 | none | redundant boolean conditional: `True if loss != metric else False` is just `loss != metric` | `self.log_metric = True if loss != metric else False` |
| P1-27 | mla/kmeans.py:135 | none | `_is_converged` returns `distance == 0` — exact float equality on a sum of euclidean distances that is essentially never bit-0; should compare against a tolerance | `return distance == 0` |

## Phase 2 — audit finding verdicts

| finding (path:line) | rule | tier | verdict | why |
|---------------------|------|------|---------|-----|
| mla/neuralnet/layers/recurrent/lstm.py:191 (+15: rnn.py:106, fm.py:76/83, optimizers.py:115/174, initializations.py:46/52/58/64, metrics/tests/test_metrics.py:60/66/72/78, examples/gbm.py:44, examples/random_forest.py:45) | 11 | indexed | real | each pair is a genuine AST-normalized T2 clone (identical body modulo one constant/callee). |
| mla/ensemble/gbm.py:79 (+3: convnet.py:9, gbm.py:79 4-sig, random_forest.py:10) | 14 | indexed | real | the param groups genuinely travel across Tree/RandomForest/GradientBoosting/Convolution signatures. |
| mla/neuralnet/nnet.py:156 (+2: layers/basic.py:41, basic.py:49) | 6 | indexed | fp | all three are property SETTERS; mutation is a setter's contract, not a dishonest query. |
| mla/kmeans.py:52 (+11: tree.py:70 x2, svm.py:129/141, metrics.py:85, initializations.py:70, datasets/base.py:7, examples/nnet_rnn_binary_add.py:20, examples/gaussian_mixture.py:12/21, examples/kmeans.py:7) | 5 | indexed | real | untyped params with an invariant established at every call site; lifts counterfactually verified. |
| mla/neuralnet/nnet.py:134 | 5 | indexed | fp | over-narrow lift `X: None` — X clearly accepts an ndarray (the `error(X)` branch); the lone default-None call site misleads inference (documented lift failure mode). |
| mla/base/base.py:50 (+10: parameters.py:49, linear_models.py:114/128/136, pca.py:35/62, naive_bayes.py:35, layers/basic.py:71, regularizers.py:17, metrics/tests/test_metrics.py:24) | 25 | indexed | fp | all are ordinary method→helper decomposition; none is a same-concept silent rename (the fit_current→fit_served ideal), so "un-greppable chain" doesn't hold. |
| mla/neuralnet/activations.py:34 (+4: initializations.py:18/22, parameters.py:83, regularizers.py:14) | 13 | indexed | real | pure forward-only wrappers (np.tanh/np.zeros/np.ones/dict.keys/self._grad) pricing a hop without adding meaning. |
| mla/ensemble/gbm.py:57 (+2: gbm.py:71, knn.py:71) | 13 | indexed | fp | leaf implementations of abstract interface methods (hess/transform/aggregate), not pass-through hops to a deeper module. |
| mla/neuralnet/optimizers.py:1 (+37 module + cost-docstring sites) | 29 | heuristic | real | detections are accurate (these modules/heavy fns carry no orienting/cost docstring); low value, report-tier, and several carry a post-import References block. |
| mla/neuralnet/nnet.py:54 (+1: lstm.py:38) | 17 | heuristic | fp | both are __init__ methods; a 1-crossing "neck" among field assignments is not a natural compute split point. |
| mla/neuralnet/layers/normalization.py:43 (+1: :104) | 18 | heuristic | real | both narrate literal `# step1..stepN` phases inside one function. |
| mla/pca.py:55 | 16 | heuristic | real | _decompose runs the SVD/variance computation then writes self.components at the tail — genuine compute-then-mutate. |
| mla/gaussian_mixture.py:13 (+2: kmeans.py:15, parameters.py:7) | 21 | heuristic | fp | the recurring expressions (`range(self.K)`, `enumerate(self.clusters)`, `self._params[item]`) are idiomatic iteration/backing-store access, not an encapsulable invariant. |
| mla/rl/dqn.py:63 | 30 | heuristic | real | `self.env.action_space.n` is a genuine 3-hop Demeter chain (into the gym API). |
| mla/neuralnet/optimizers.py:18 (+8: optimizers.py:39/60, gbm.py:34/44, constraints.py:17, tree.py:193/202, dqn.py:145) | 22 | heuristic | real | each uses only the class's public interface (Meyers velcro); base-Optimizer trio has no instance state at all. Report-ranked; template-method cases low value. |
| mla/neuralnet/initializations.py:18 (+6: :22/46/52/58/64, examples/gaussian_mixture.py:12) | 1 | heuristic | real | opaque **kwargs on public boundaries; initializers silently swallow scale/kwargs. |
| mla/base/base.py:57 | 2 | heuristic | real | `self.X is not None` is always True given X's inferred ndarray type — the fit-guard never triggers (genuinely redundant/ineffective). |
| mla/metrics/metrics.py:88 (+3: activations.py:62, initializations.py:73, loss.py:9) | 24 | heuristic | real | `globals()[name]` dispatch — unfindable by search, blinds whole-program analysis. |
| mla/base/__init__.py:2 (+10: datasets/__init__.py:2, metrics/__init__.py:2, layers/__init__.py:2/3/4, recurrent/__init__.py:2/3, utils/__init__.py:3, tests/test_activations.py:5, tests/test_optimizers.py:7) | 26 | heuristic | real | genuine star imports that hide the exported surface. |
| mla/neuralnet/layers/basic.py:171 (+1: convnet.py:132) | 15 | heuristic | fp | the params are numpy arrays used via .reshape/.shape; narrowing an ndarray to a reshape/shape protocol is not meaningful demand-narrowing. |
| mla/neuralnet/optimizers.py:65 | 7 | heuristic | real | Optimizer.setup docstring states "Must be called before optimization process" — a narrated protocol that wants encoding. |

## Phase 3 — reconciliation

| P1 id | rule | class (covered / detector-miss / threshold-miss / inventory-gap) | note |
|-------|------|------------------------------------------------------------------|------|
| P1-1 | #19 | detector-miss | #19 never fired in the audit; `row in cluster` + `.remove` in a nested loop not detected. |
| P1-2 | #19 | detector-miss | same rule absent; `replay.pop(0)` in a while loop not detected. |
| P1-3 | #11 | threshold-miss | clone detector ran but did not pair get_split_mask/split (below T2 similarity). |
| P1-4 | #11 | threshold-miss | #11 paired the optimizer setup() methods, not the update() scaffold I cited (update bodies exceed the T2 diff). |
| P1-5 | #18 | covered | fired at normalization.py:43. |
| P1-6 | #18 | covered | fired at normalization.py:104. |
| P1-7 | #24 | covered | fired at metrics.py:88. |
| P1-8 | #24 | covered | fired at activations.py:62. |
| P1-9 | #24 | covered | fired at loss.py:9. |
| P1-10 | #24 | covered | fired at initializations.py:73. |
| P1-11 | #26 | covered | fired at layers/__init__.py:2-4. |
| P1-12 | #26 | covered | fired at metrics/__init__.py:2. |
| P1-13 | #26 | covered | fired at base/__init__.py:2. |
| P1-14 | #1 | covered | fired at initializations.py:18 (+ siblings). |
| P1-15 | #14 | covered | subsumed by the (max_depth,max_features,min_samples_split) clump whose 6 sigs include Tree._train/train (anchored gbm.py:79). |
| P1-16 | #12 | detector-miss | #12 never fired; svm.clip clamp reimpl absent from the idiom catalog. |
| P1-17 | #13 | covered | fired at regularizers.py:14 (Regularizer.grad). |
| P1-18 | #9 | detector-miss | #9 never fired; module-level np.random.seed (a call, not a mutated module global) is outside its model. |
| P1-19 | #2 | detector-miss | oracle won't ground `type(a)!=type(b)` (equality-never-grounded trap); control-flow redundancy after coercion is out of #2's reach. |
| P1-20 | none | inventory-gap | loss_grad-set-after-training ordering bug — no rule (site was flagged #11 for an unrelated clone). |
| P1-21 | none | inventory-gap | SGD Nesterov no-op recomputation — no rule. |
| P1-22 | none | inventory-gap | hinge np.max axis-arg misuse — no rule. |
| P1-23 | none | inventory-gap | LSTM.backward_pass returns all-zero output — no rule. |
| P1-24 | none | inventory-gap | dead `dataset="training"` default — no rule. |
| P1-25 | none | inventory-gap | misspelled `convoltuion_shape` propagated to all callers — no rule. |
| P1-26 | none | inventory-gap | redundant `True if … else False` — no rule. |
| P1-27 | none | inventory-gap | float `== 0` convergence check — no rule. |
