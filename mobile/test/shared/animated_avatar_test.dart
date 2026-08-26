import 'package:maju/shared/animated_avatar.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  const posterUrl = 'https://relay.example/media/poster.png';
  const animationUrl = 'https://relay.example/media/animation.png?loop=1';

  test('parses the selected poster and animated PNG URLs', () {
    final url = buildAnimatedAvatarUrl(posterUrl, animationUrl);

    final parsed = parseAnimatedAvatarUrl(url);

    expect(parsed?.posterUrl, posterUrl);
    expect(parsed?.animationUrl, animationUrl);
  });

  test('builds the shared desktop and mobile fragment format', () {
    expect(
      buildAnimatedAvatarUrl(posterUrl, animationUrl),
      '$posterUrl#maju-anim=${Uri.encodeComponent(animationUrl)}',
    );
  });

  test('rejects malformed and non-http animated avatar URLs', () {
    expect(parseAnimatedAvatarUrl(posterUrl), isNull);
    expect(parseAnimatedAvatarUrl('$posterUrl#maju-anim='), isNull);
    expect(parseAnimatedAvatarUrl('$posterUrl#maju-anim=%E0%A4%A'), isNull);
    expect(parseAnimatedAvatarUrl('$posterUrl#maju-anim=%'), isNull);
    expect(
      parseAnimatedAvatarUrl(
        '$posterUrl#maju-anim=${Uri.encodeComponent('javascript:alert(1)')}',
      ),
      isNull,
    );
    expect(
      parseAnimatedAvatarUrl(
        'data:image/png;base64,xx#maju-anim='
        '${Uri.encodeComponent(animationUrl)}',
      ),
      isNull,
    );
  });
}
