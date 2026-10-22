package com.codync.android

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.codync.android.design.*

/**
 * A section's heading, drawn like iOS `ComputerSection`: its name and bot count, their unread while
 * folded, and the chevron. Tap folds it; touch and hold opens [actions] (none for a host without sections).
 */
@Composable internal fun SectionHeader(header: RosterLine.Header, toggle: () -> Unit, actions: List<MenuItem>, modifier: Modifier = Modifier) {
    val turn by animateFloatAsState(if (header.folded) -90f else 0f, Motion.layout(), label = "Chevron")
    val tap = if (actions.isEmpty()) Modifier.clickable(role = Role.Button, onClick = toggle)
        else Modifier.contextActions(onClick = toggle) { actions }
    Row(modifier.fillMaxWidth().padding(top = 14.dp, bottom = 2.dp).then(tap)
        // One spoken line for the whole header (the count and the badge say nothing alone): clear the parts, then describe it.
        .clearAndSetSemantics {
            heading()
            stateDescription = if (header.folded) "Collapsed" else "Expanded"
            contentDescription = buildString {
                append("${header.name}, ${header.count} ${if (header.count == 1) "bot" else "bots"}")
                if (header.folded && header.unread > 0) append(", ${header.unread} unread")
            }
        }
        .padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
        Row(Modifier.weight(1f), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(header.name, Modifier.weight(1f, fill = false), style = Type.footnote.semibold, color = Palette.secondary,
                maxLines = 1, overflow = TextOverflow.Ellipsis)
            Text("${header.count}", style = Type.footnote, color = Palette.tertiary)
            AnimatedVisibility(header.folded && header.unread > 0, enter = fadeIn(Motion.fade()), exit = fadeOut(Motion.fade())) {
                UnreadBadge(header.unread)
            }
        }
        Spacer(Modifier.width(8.dp))
        Symbol("chevron.down", Modifier.rotate(turn), size = 14.dp, tint = Palette.tertiary)
    }
}
