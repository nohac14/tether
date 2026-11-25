# Dedicated scroll engines

Each strip follows one captured contact and measures only its own axis. The
horizontal strip uses X and the vertical strip uses Y. The neutral corner
does not capture input. The vertical strip keeps its axis on either side of the
pad. Contacts crossing another region continue on their original axis.

One normalized unit is provisionally 48 CSS pixels at 1× gain. Scroll deltas
remain fractional until fixed-point conversion by the client dispatcher.
Positive normalized vertical and horizontal values request viewport movement
up and right, respectively, matching the Windows wheel conventions. Natural
direction moves content with the finger. Traditional reverses that sign. Each
axis has independent direction, sensitivity (0.125× to 8×), and momentum.
Native translation and per-platform qualification follow the
[input normalization contract](input-normalization.md).

Each strip retains at most eight recent axis positions within an 80 ms velocity
window. Reversing direction resets the estimator. The velocity denominator is
at least 8 ms and the resulting velocity is capped at 2.5 CSS px/ms. A release
requires movement in the last 60 ms to start a coast. Steps over 160 CSS pixels
and sample gaps over 100 ms are discarded and re-anchor the axis there.

| Momentum | Exponential time constant | Maximum coast |
| --- | --- | --- |
| Off | None | 0 ms |
| Low | 85 ms | 250 ms |
| Medium (default) | 140 ms | 480 ms |
| High | 215 ms | 700 ms |

Momentum integrates the decay over frame time and ends early below
0.04 CSS px/ms. A new surface touch, including one that starts on the neutral
corner, stops existing momentum. Cancellation, viewport or ownership changes,
and browser suspension clear momentum and active contacts. Each coasting strip
also independently discards work following a frame gap over 100 ms. Anomaly
handling accumulates no deferred jump. Physical-device tuning remains pending.
