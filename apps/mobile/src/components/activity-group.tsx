import type { ActivityFileChange, ActivityItem, ActivityKind, TranscriptBlock } from '@padu/client';
import { activitiesForBlock } from '@padu/client/event-reducer';
import {
  ACTIVITY_PREVIEW_LIMIT,
  activityActionLabel,
  activityDisclosureSections,
  activityFileChangeStats,
  activityHeaderTitle,
  activityPreview,
  activityPreviewWindow,
  activityRowDetail,
} from '@padu/client/transcript-presentation';
import type { SymbolViewProps } from 'expo-symbols';
import { memo, useEffect, useState } from 'react';
import { Image, Pressable, StyleSheet, Text, View } from 'react-native';

import { AppSymbol } from './app-symbol';
import { DiffView } from './diff-view';
import { MonoFont, NativeTint } from '@/constants/theme';
import { useTheme } from '@/hooks/use-theme';

const ACTIVITY_ICONS: Record<ActivityKind, SymbolViewProps['name']> = {
  reasoning: { ios: 'sparkles', android: 'auto_awesome', web: 'auto_awesome' },
  command: { ios: 'terminal', android: 'terminal', web: 'terminal' },
  fileChange: { ios: 'pencil.line', android: 'edit', web: 'edit' },
  fileRead: { ios: 'doc.text', android: 'description', web: 'description' },
  fileSearch: { ios: 'doc.text.magnifyingglass', android: 'find_in_page', web: 'find_in_page' },
  fileList: { ios: 'folder', android: 'folder', web: 'folder' },
  search: { ios: 'globe', android: 'travel_explore', web: 'travel_explore' },
  plan: { ios: 'checklist', android: 'checklist', web: 'checklist' },
  tool: { ios: 'wrench.and.screwdriver', android: 'build', web: 'build' },
};

/**
 * Desktop's activity treatment: the group collapses to a single summary line
 * (the live activity's title while streaming, "Ran N commands · …" once
 * settled), auto-expanded only while live. Expanded rows hang off a left rail
 * as bordered cards headed by a bold action label.
 */
export const ActivityGroup = memo(function ActivityGroup({
  block,
  live,
}: {
  block: TranscriptBlock;
  live: boolean;
}) {
  const theme = useTheme();
  const activities = activitiesForBlock(block);
  const [expanded, setExpanded] = useState(live);
  const [showAll, setShowAll] = useState(false);
  useEffect(() => {
    setExpanded(live);
  }, [live]);
  if (!activities.length) return null;
  const preview = activityPreviewWindow(activities.length, live, showAll);
  const visible = activities.slice(preview.start, preview.end);
  const hiddenCount = activities.length - (preview.end - preview.start);
  return (
    <View style={styles.group}>
      <Pressable
        accessibilityRole="button"
        accessibilityState={{ expanded }}
        onPress={() => setExpanded((value) => !value)}
        style={({ pressed }) => [styles.groupHeader, { opacity: pressed ? 0.6 : 1 }]}>
        <Text numberOfLines={1} style={[styles.groupTitle, { color: theme.textSecondary }]}>
          {activityHeaderTitle(activities, live)}
        </Text>
        <AppSymbol
          name={expanded
            ? { ios: 'chevron.down', android: 'keyboard_arrow_down', web: 'keyboard_arrow_down' }
            : { ios: 'chevron.right', android: 'chevron_right', web: 'chevron_right' }}
          size={10}
          tintColor={theme.textGhost}
        />
      </Pressable>
      {expanded && (
        <View style={[styles.rail, { borderLeftColor: theme.border }]}>
          {visible.map((activity) => (
            <ActivityRow activity={activity} key={activity.id} />
          ))}
          {activities.length > ACTIVITY_PREVIEW_LIMIT && (
            <Pressable
              accessibilityRole="button"
              accessibilityState={{ expanded: showAll }}
              onPress={() => setShowAll((value) => !value)}
              style={({ pressed }) => [styles.showAll, { opacity: pressed ? 0.6 : 1 }]}>
              <Text numberOfLines={1} style={[styles.showAllLabel, { color: theme.textSecondary }]}>
                {showAll ? 'Show fewer activities' : `Show ${hiddenCount} more activities`}
              </Text>
              <AppSymbol
                name={showAll
                  ? { ios: 'chevron.up', android: 'keyboard_arrow_up', web: 'keyboard_arrow_up' }
                  : { ios: 'chevron.down', android: 'keyboard_arrow_down', web: 'keyboard_arrow_down' }}
                size={10}
                tintColor={theme.textGhost}
              />
            </Pressable>
          )}
        </View>
      )}
    </View>
  );
});

function ActivityRow({ activity }: { activity: ActivityItem }) {
  const theme = useTheme();
  const reasoningContent = activity.reasoning?.content.trim() ?? '';
  const sections = activity.reasoning ? [] : activityDisclosureSections(activity);
  const changes = activity.file_changes ?? [];
  const images = activity.image_urls ?? [];
  const hasDetail = Boolean(reasoningContent || sections.length || changes.length || images.length);
  const [expanded, setExpanded] = useState(Boolean(activity.reasoning && !activity.complete));
  const reasoningRow = Boolean(activity.reasoning);
  useEffect(() => {
    // Thinking stays open while it streams and folds shut when it completes.
    if (reasoningRow) setExpanded(!activity.complete);
  }, [activity.complete, reasoningRow]);
  const preview = expanded || reasoningRow ? '' : activityPreview(activity);
  const rowDetail = activityRowDetail(activity) || preview;
  const fileStats = activityFileChangeStats(activity);

  return (
    <View style={[styles.card, { backgroundColor: theme.surface, borderColor: theme.border }]}>
      <Pressable
        accessibilityRole={hasDetail ? 'button' : 'text'}
        accessibilityState={hasDetail ? { expanded } : undefined}
        disabled={!hasDetail}
        onPress={() => setExpanded((value) => !value)}
        style={({ pressed }) => [
          styles.cardHeader,
          { backgroundColor: pressed && hasDetail ? theme.overlay : 'transparent' },
        ]}>
        <AppSymbol
          name={ACTIVITY_ICONS[activity.kind]}
          size={12}
          tintColor={theme.textTertiary}
        />
        <Text style={[styles.actionLabel, { color: activity.failed ? theme.danger : theme.textSecondary }]}>
          {activityActionLabel(activity)}
        </Text>
        {rowDetail ? (
          <Text numberOfLines={1} style={[styles.rowDetail, { color: theme.textSecondary }]}>
            {rowDetail}
          </Text>
        ) : (
          <View style={styles.rowSpacer} />
        )}
        {fileStats && (
          <Text style={styles.stats}>
            <Text style={{ color: theme.success }}>+{fileStats.additions}</Text>
            <Text style={{ color: theme.textGhost }}> </Text>
            <Text style={{ color: theme.danger }}>−{fileStats.deletions}</Text>
          </Text>
        )}
        <ActivityState activity={activity} expanded={expanded} hasDetail={hasDetail} />
      </Pressable>
      {expanded && hasDetail && (
        <View style={[styles.cardBody, { borderTopColor: theme.border }]}>
          {reasoningRow ? (
            <Text selectable style={[styles.reasoningText, { color: theme.textSecondary }]}>
              {plainReasoning(reasoningContent)}
            </Text>
          ) : (
            <>
              {sections.map((section) => (
                <View key={section.kind} style={styles.section}>
                  {section.label && (
                    <Text style={[styles.sectionLabel, { color: theme.textTertiary }]}>
                      {section.label}
                    </Text>
                  )}
                  {section.content ? (
                    <Text selectable style={[styles.monoText, { color: theme.textSecondary }]}>
                      {boundedText(section.content)}
                    </Text>
                  ) : null}
                </View>
              ))}
              {changes.map((change) => (
                <FileChangeRow change={change} key={change.path} />
              ))}
              {images.map((url, index) => (
                <Image
                  key={index}
                  resizeMode="contain"
                  source={{ uri: url }}
                  style={[styles.image, { backgroundColor: theme.inset }]}
                />
              ))}
            </>
          )}
        </View>
      )}
    </View>
  );
}

/** Mirrors the desktop row's trailing state: chevron when there is detail,
 * nothing for finished reasoning, an alert for failures, a dot while live. */
function ActivityState({
  activity,
  expanded,
  hasDetail,
}: {
  activity: ActivityItem;
  expanded: boolean;
  hasDetail: boolean;
}) {
  const theme = useTheme();
  if (hasDetail) {
    return (
      <AppSymbol
        name={expanded
          ? { ios: 'chevron.down', android: 'keyboard_arrow_down', web: 'keyboard_arrow_down' }
          : { ios: 'chevron.right', android: 'chevron_right', web: 'chevron_right' }}
        size={10}
        tintColor={theme.textGhost}
      />
    );
  }
  if (activity.reasoning) return null;
  if (activity.failed) {
    return (
      <AppSymbol
        name={{ ios: 'exclamationmark.triangle.fill', android: 'warning', web: 'warning' }}
        size={12}
        tintColor={theme.danger}
      />
    );
  }
  if (activity.complete) return null;
  return <View accessibilityLabel="Running" style={[styles.runningDot, { backgroundColor: NativeTint }]} />;
}

function FileChangeRow({ change }: { change: ActivityFileChange }) {
  const theme = useTheme();
  const [open, setOpen] = useState(false);
  const hasDiff = Boolean(change.diff?.trim());
  const statusColor = change.status === 'added'
    ? theme.success
    : change.status === 'deleted'
      ? theme.danger
      : theme.warning;
  return (
    <View style={styles.fileChange}>
      <Pressable
        accessibilityRole={hasDiff ? 'button' : 'text'}
        accessibilityState={hasDiff ? { expanded: open } : undefined}
        disabled={!hasDiff}
        onPress={() => setOpen((value) => !value)}
        style={({ pressed }) => [styles.fileRow, { opacity: pressed ? 0.6 : 1 }]}>
        <View style={[styles.statusDot, { backgroundColor: statusColor }]} />
        <Text numberOfLines={1} style={[styles.filePath, { color: theme.text }]}>
          {change.path}
        </Text>
        {(change.additions != null || change.deletions != null) && (
          <Text style={styles.stats}>
            <Text style={{ color: theme.success }}>+{change.additions ?? 0}</Text>
            <Text style={{ color: theme.textGhost }}> </Text>
            <Text style={{ color: theme.danger }}>−{change.deletions ?? 0}</Text>
          </Text>
        )}
        {hasDiff && (
          <AppSymbol
            name={open
              ? { ios: 'chevron.down', android: 'keyboard_arrow_down', web: 'keyboard_arrow_down' }
              : { ios: 'chevron.right', android: 'chevron_right', web: 'chevron_right' }}
            size={10}
            tintColor={theme.textGhost}
          />
        )}
      </Pressable>
      {open && change.diff ? <DiffView diff={change.diff} /> : null}
    </View>
  );
}

function boundedText(value: string): string {
  const limit = 12_000;
  return value.length <= limit ? value : `${value.slice(0, limit)}\n\n… Output truncated`;
}

/** Reasoning is throwaway thinking: render it as quiet plain text, never
 * heavier than the answer. Strips the markdown emphasis and heading markers
 * providers put on their summary headlines. */
function plainReasoning(value: string): string {
  return boundedText(value)
    .replace(/\*\*([^*]+)\*\*/g, '$1')
    .replace(/__([^_]+)__/g, '$1')
    .replace(/^#{1,6}\s+/gm, '');
}

const styles = StyleSheet.create({
  group: { marginBottom: 10 },
  groupHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 6,
    minHeight: 28,
  },
  groupTitle: { flexShrink: 1, fontSize: 13, fontWeight: '500' },
  rail: {
    borderLeftWidth: StyleSheet.hairlineWidth,
    gap: 7,
    marginLeft: 4,
    marginTop: 2,
    paddingBottom: 2,
    paddingLeft: 11,
  },
  showAll: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 6,
    minHeight: 24,
    paddingHorizontal: 8,
  },
  showAllLabel: { flexShrink: 1, fontSize: 12.5, fontWeight: '500' },
  card: {
    borderRadius: 9,
    borderWidth: StyleSheet.hairlineWidth,
    overflow: 'hidden',
  },
  cardHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 8,
    minHeight: 32,
    paddingHorizontal: 9,
  },
  actionLabel: { fontSize: 12.5, fontWeight: '600' },
  rowDetail: { flex: 1, fontSize: 12.5 },
  rowSpacer: { flex: 1 },
  stats: { fontSize: 11.5, fontVariant: ['tabular-nums'], fontWeight: '600' },
  runningDot: { borderRadius: 3, height: 6, width: 6 },
  cardBody: {
    borderTopWidth: StyleSheet.hairlineWidth,
    gap: 8,
    paddingHorizontal: 10,
    paddingVertical: 8,
  },
  reasoningText: { fontSize: 12.5, lineHeight: 18 },
  section: { gap: 3 },
  sectionLabel: { fontSize: 11, fontWeight: '600' },
  monoText: { fontFamily: MonoFont, fontSize: 11, lineHeight: 16 },
  fileChange: { gap: 6 },
  fileRow: { alignItems: 'center', flexDirection: 'row', gap: 7, minHeight: 24 },
  statusDot: { borderRadius: 3, height: 6, width: 6 },
  filePath: { flex: 1, fontFamily: MonoFont, fontSize: 11 },
  image: { borderRadius: 8, height: 200, width: '100%' },
});
