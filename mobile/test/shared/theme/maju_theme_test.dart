import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:maju/shared/theme/theme.dart';
import 'package:maju/shared/widgets/frosted_app_bar.dart';

void main() {
  group('Maju theme catalog entries', () {
    test('both halves are in the catalog', () {
      expect(findTheme(majuThemeName), isNotNull);
      expect(findTheme(majuDarkThemeName), isNotNull);
    });

    test('borrow the GitHub palettes', () {
      final maju = findTheme(majuThemeName)!;
      final github = findTheme('github-light')!;
      expect(maju.bg, github.bg);
      expect(maju.fg, github.fg);
      expect(maju.comment, github.comment);

      final majuDark = findTheme(majuDarkThemeName)!;
      final githubDark = findTheme('github-dark')!;
      expect(majuDark.bg, githubDark.bg);
      expect(majuDark.fg, githubDark.fg);
      expect(majuDark.comment, githubDark.comment);
    });

    test('are a light/dark pair', () {
      expect(findTheme(majuThemeName)!.isDark, isFalse);
      expect(findTheme(majuDarkThemeName)!.isDark, isTrue);
      expect(themePairFor(majuThemeName), majuDarkThemeName);
      expect(themePairFor(majuDarkThemeName), majuThemeName);
    });

    test('appear as a single System-mode option labelled "Maju"', () {
      final paired = themeGroups().paired.map((t) => t.name);
      expect(paired, contains(majuThemeName));
      expect(paired, isNot(contains(majuDarkThemeName)));
      expect(pairedThemeLabel(majuThemeName), 'Maju');
      expect(themeSelectionLabel(majuThemeName, ThemeMode.system), 'Maju');
      expect(themeSelectionLabel(majuDarkThemeName, ThemeMode.system), 'Maju');
    });

    test('resolve across brightnesses like any other pair', () {
      final resolved = resolveSchemes(majuThemeName, ThemeMode.system);
      expect(resolved.forcedMode, isNull);
      expect(resolved.light.brightness, Brightness.light);
      expect(resolved.dark.brightness, Brightness.dark);
      expect(resolved.lightTheme?.name, majuThemeName);
      expect(resolved.darkTheme?.name, majuDarkThemeName);

      expect(
        effectiveTheme(majuThemeName, ThemeMode.dark)?.name,
        majuDarkThemeName,
      );
      expect(
        effectiveTheme(majuDarkThemeName, ThemeMode.light)?.name,
        majuThemeName,
      );
    });

    test(
      'fallbacks expose the effective Maju theme for gradient selection',
      () {
        final coerced = resolveSchemes('nord', ThemeMode.light);
        expect(coerced.lightTheme?.name, majuThemeName);
        expect(
          majuTopSectionGradient(
            coerced.lightTheme!.name,
            coerced.light.brightness,
          ),
          isNotNull,
        );

        final unknown = resolveSchemes('not-a-theme', ThemeMode.light);
        expect(unknown.lightTheme?.name, majuThemeName);
        expect(
          majuTopSectionGradient(
            unknown.lightTheme!.name,
            unknown.light.brightness,
          ),
          isNotNull,
        );
      },
    );
  });

  group('majuTopSectionGradient', () {
    test('is null for non-Maju themes', () {
      expect(majuTopSectionGradient('github-light', Brightness.light), isNull);
      expect(majuTopSectionGradient('nord', Brightness.dark), isNull);
    });

    test('paints top to bottom for both halves of the pair', () {
      for (final name in [majuThemeName, majuDarkThemeName]) {
        final gradient = majuTopSectionGradient(name, Brightness.light);
        expect(gradient, isNotNull, reason: '$name should be gradient-backed');
        expect(gradient!.begin, Alignment.topCenter);
        expect(gradient.end, Alignment.bottomCenter);
        expect(gradient.colors, hasLength(2));
      }
    });

    test('brightness selects the stops, not the theme name', () {
      // Both halves enable the gradient, so System mode keeps it on across an
      // OS switch — the applied brightness alone decides which stops are used.
      final light = majuTopSectionGradient(majuThemeName, Brightness.light)!;
      final dark = majuTopSectionGradient(majuThemeName, Brightness.dark)!;

      expect(light.colors, isNot(dark.colors));
      expect(
        majuTopSectionGradient(majuDarkThemeName, Brightness.dark)!.colors,
        dark.colors,
      );
      expect(
        majuTopSectionGradient(majuDarkThemeName, Brightness.light)!.colors,
        light.colors,
      );
    });

    test('is opaque so the color replaces the frosted fill', () {
      for (final brightness in Brightness.values) {
        final gradient = majuTopSectionGradient(majuThemeName, brightness)!;
        for (final color in gradient.colors) {
          expect(color.a, 1.0);
        }
      }
    });
  });

  group('theme threading', () {
    BoxDecoration barDecoration(WidgetTester tester) {
      final container = tester
          .widgetList<Container>(
            find.descendant(
              of: find.byType(FrostedAppBar),
              matching: find.byType(Container),
            ),
          )
          .first;
      return container.decoration! as BoxDecoration;
    }

    Widget harness(ThemeData theme) => MaterialApp(
      theme: theme,
      home: Builder(
        builder: (context) => Stack(
          children: [
            FrostedAppBar(
              gradient: context.appColors.topSectionGradient,
              title: const Text('Home'),
            ),
          ],
        ),
      ),
    );

    testWidgets('AppTheme carries the gradient to the top section', (
      tester,
    ) async {
      await tester.pumpWidget(
        harness(
          AppTheme.light(
            topSectionGradient: majuTopSectionGradient(
              majuThemeName,
              Brightness.light,
            ),
          ),
        ),
      );

      final decoration = barDecoration(tester);
      expect(decoration.gradient, isNotNull);
      // A BoxDecoration cannot paint a color and a gradient at once.
      expect(decoration.color, isNull);
    });

    testWidgets('non-Maju themes keep the frosted surface fill', (
      tester,
    ) async {
      await tester.pumpWidget(harness(AppTheme.light()));

      final decoration = barDecoration(tester);
      expect(decoration.gradient, isNull);
      expect(decoration.color, isNotNull);
    });
  });

  group('isMajuTheme', () {
    test('matches only the Maju pair', () {
      expect(isMajuTheme(majuThemeName), isTrue);
      expect(isMajuTheme(majuDarkThemeName), isTrue);
      expect(isMajuTheme('github-light'), isFalse);
      expect(isMajuTheme(''), isFalse);
    });
  });
}
