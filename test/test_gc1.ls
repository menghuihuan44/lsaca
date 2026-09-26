any a = 1;
any b = 2;
any c = a + b;
print "gc01 a + b =";
print c;

print "gc02 const string pad";
print "gc03 const string pad";
print "gc04 const string pad";
print "gc05 const string pad";
print "gc06 const string pad";
print "gc07 const string pad";
print "gc08 const string pad";
print "gc09 const string pad";
print "gc10 const string pad";

func add2(any x, any y) -> any {
    return x + y;
}

print "gc11 call add2:";
print add2(3, 4);

any i = 0;
for (i = 0; i < 3; i = i + 1) {
    print "gc12 loop i:";
    print i;
}

{
    any d = add2(10, 20);
    print "gc13 nested scope:";
    print d;
}

print "gc14 done";