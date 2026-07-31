import 'package:flutter/material.dart';

/// Name of the first-party Maju theme. Maju reuses the GitHub Light palette for
/// every base color; the one thing that sets it apart is a branded gradient
/// painted across the app's top section. Mirrors desktop, where the same
/// gradient fills the sidebar canvas — see `data-maju-sidebar` in
/// `desktop/src/shared/styles/globals/theme.css`.
const majuThemeName = 'maju';

/// Name of the dark counterpart, which reuses the GitHub Dark palette and the
/// dark-tuned gradient stops. Paired with [majuThemeName] in `themePairs`, so
/// the two behave as a single "Maju" choice under System mode.
const majuDarkThemeName = 'maju-dark';

/// Whether [themeName] is either half of the Maju pair. Both halves enable the
/// gradient so System mode keeps it on across an OS light/dark switch.
bool isMajuTheme(String themeName) =>
    themeName == majuThemeName || themeName == majuDarkThemeName;

/// Gradient stops, matching desktop's `--maju-gradient-*` custom properties.
const _lightTop = Color(0xFFE6E6B6);
const _lightBottom = Color(0xFFC4D0DA);
const _darkTop = Color(0xFF4A4616);
const _darkBottom = Color(0xFF0A1423);

/// The Maju gradient for the app's top section, or null when [themeName] is not
/// a Maju theme — in which case the section keeps its default frosted fill.
///
/// The stops are fully opaque: under Maju the color replaces the frosted
/// treatment rather than tinting it, matching desktop's solid sidebar canvas.
///
/// [brightness] comes from the applied color scheme rather than the theme name,
/// so System mode picks the right stops as the OS switches.
LinearGradient? majuTopSectionGradient(
  String themeName,
  Brightness brightness,
) {
  if (!isMajuTheme(themeName)) return null;

  final isDark = brightness == Brightness.dark;
  return LinearGradient(
    begin: Alignment.topCenter,
    end: Alignment.bottomCenter,
    colors: [
      isDark ? _darkTop : _lightTop,
      isDark ? _darkBottom : _lightBottom,
    ],
  );
}
