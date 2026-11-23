import re,sys
EXPECT={"set_pizza_size","set_pizza_style","toggle_layer","add_topping","remove_topping","manage_pizza","share_pizza"}
n=0; got=set()
for line in open(sys.argv[1],encoding='utf8'):
    if '"method":"WebMCP.toolsAdded"' in line:
        n+=1
        got.update(re.findall(r'\{"name":"([^"]+)","description"', line))
ok=(got-{"zz"})==EXPECT
print(f"file={sys.argv[1]} toolsAdded_events={n} names={sorted(got)} match_expected_7={ok}")
sys.exit(0 if ok else 1)
