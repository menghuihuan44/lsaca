for (any i = 0; i < 5; i = i + 1) {
    if (i == 1) {
        continue;
    }
    if (i == 3) {
        break;
    }
    print "for loop i:";
    print i;
}

for (any x = 0; x < 2; x = x + 1) {
    for (any y = 0; y < 3; y = y + 1) {
        if (y == 1) {
            continue;
        }
        if (x == 1) {
            break;
        }
        print "nested loop x, y:";
        print x;
        print y;
    }
}