any a = 1;
print "==== a ====";
print a;
{
    any b = 2;
    print "==== b ====";
    print b;
    {
        any c = a + b;
        print "==== c ====";
        print c;
        /* It should be an error. */
        /*
        any a = a + 99;
        print "==== a ====";
        print a;
        */
        /* Yes, it is an error. My language handle it correctly. */
    }
    any d = 3;
    print "==== d ====";
    print d;
}
any e = 4;
print "==== e ====";
print e;

/* It should be an error. */
/* 
any a = 99; 
*/
/* Yes, it is an error. My language handle it correctly. */


/* It should be an error. */
/*
any f = f;
print "==== f ====";
print f;
*/
/* Yes, it is an error. My language handle it correctly. */